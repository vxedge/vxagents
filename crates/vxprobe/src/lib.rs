//! vxprobe core: a reflector that echoes UDP packets, and a sender that
//! paces sequenced probes, correlates echoes, and reports RTT percentiles,
//! jitter, and loss.
//!
//! Timestamping: monotonic userspace clock (`Instant`) around send/receive.
//! Documented limitation: kernel timestamps (SO_TIMESTAMPING) are a recorded
//! follow-up (TASKS.md) — userspace clocks add scheduler noise to the tails,
//! which is acceptable for slice-vs-slice comparison but must be stated in
//! results metadata.

use hdrhistogram::Histogram;
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use vxagent_common::{packet, prom};

/// Echo every received datagram back to its sender, verbatim.
pub async fn run_reflector(listen: SocketAddr) -> std::io::Result<()> {
    let sock = UdpSocket::bind(listen).await?;
    tracing::info!(addr = %sock.local_addr()?, "vxprobe reflector listening");
    let mut buf = vec![0u8; 65536];
    loop {
        let (n, peer) = sock.recv_from(&mut buf).await?;
        // Best effort: a lost echo is measured as loss by the sender.
        let _ = sock.send_to(&buf[..n], peer).await;
    }
}

#[derive(Debug, Clone)]
pub struct SendConfig {
    pub target: SocketAddr,
    pub pps: u64,
    pub payload_bytes: usize,
    pub duration: Duration,
    /// A probe unanswered for this long counts as lost.
    pub timeout: Duration,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub sent: u64,
    pub received: u64,
    pub lost: u64,
    pub loss_pct: f64,
    pub rtt_us: RttSummary,
}

#[derive(Debug, Serialize)]
pub struct RttSummary {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub mean: f64,
    /// Jitter reported as RTT standard deviation.
    pub stddev: f64,
    pub max: f64,
}

/// Shared live state, rendered by the Prometheus exporter during a run.
/// std::sync mutexes on purpose: critical sections are tiny and lock-free
/// counters carry the hot-path totals.
pub struct ProbeState {
    sent: AtomicU64,
    received: AtomicU64,
    lost: AtomicU64,
    // RTTs recorded in nanoseconds, 3 significant figures, up to 60 s.
    histogram: Mutex<Histogram<u64>>,
    pending: Mutex<HashMap<u64, Instant>>,
}

impl ProbeState {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            sent: AtomicU64::new(0),
            received: AtomicU64::new(0),
            lost: AtomicU64::new(0),
            histogram: Mutex::new(
                Histogram::new_with_bounds(1, 60_000_000_000, 3).expect("valid histogram bounds"),
            ),
            pending: Mutex::new(HashMap::new()),
        })
    }

    fn lock_histogram(&self) -> std::sync::MutexGuard<'_, Histogram<u64>> {
        self.histogram.lock().expect("histogram lock poisoned")
    }

    fn lock_pending(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Instant>> {
        self.pending.lock().expect("pending lock poisoned")
    }

    /// Prometheus text exposition of the live state.
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(512);
        prom::line(
            &mut out,
            "vxprobe_sent_total",
            &[],
            self.sent.load(Ordering::Relaxed) as f64,
        );
        prom::line(
            &mut out,
            "vxprobe_received_total",
            &[],
            self.received.load(Ordering::Relaxed) as f64,
        );
        prom::line(
            &mut out,
            "vxprobe_lost_total",
            &[],
            self.lost.load(Ordering::Relaxed) as f64,
        );
        let h = self.lock_histogram();
        if !h.is_empty() {
            for (q, label) in [(0.50, "0.5"), (0.95, "0.95"), (0.99, "0.99")] {
                prom::line(
                    &mut out,
                    "vxprobe_rtt_seconds",
                    &[("quantile", label)],
                    h.value_at_quantile(q) as f64 / 1e9,
                );
            }
            prom::line(&mut out, "vxprobe_rtt_seconds_mean", &[], h.mean() / 1e9);
            prom::line(&mut out, "vxprobe_rtt_seconds_stddev", &[], h.stdev() / 1e9);
        }
        out
    }

    pub fn summary(&self) -> Summary {
        let sent = self.sent.load(Ordering::Relaxed);
        let received = self.received.load(Ordering::Relaxed);
        let lost = self.lost.load(Ordering::Relaxed);
        let h = self.lock_histogram();
        let us = |v: u64| v as f64 / 1e3;
        Summary {
            sent,
            received,
            lost,
            loss_pct: if sent > 0 {
                lost as f64 * 100.0 / sent as f64
            } else {
                0.0
            },
            rtt_us: RttSummary {
                p50: us(h.value_at_quantile(0.50)),
                p95: us(h.value_at_quantile(0.95)),
                p99: us(h.value_at_quantile(0.99)),
                mean: h.mean() / 1e3,
                stddev: h.stdev() / 1e3,
                max: us(h.max()),
            },
        }
    }
}

/// Run a probe session against a reflector; returns the final summary.
pub async fn run_sender(cfg: SendConfig, state: Arc<ProbeState>) -> std::io::Result<Summary> {
    let sock = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    sock.connect(cfg.target).await?;

    // Receiver: correlate echoes with pending sends.
    let recv_state = state.clone();
    let recv_sock = sock.clone();
    let receiver = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            let Ok(n) = recv_sock.recv(&mut buf).await else {
                return;
            };
            let now = Instant::now();
            if let Some(seq) = packet::decode(&buf[..n])
                && let Some(sent_at) = recv_state.lock_pending().remove(&seq)
            {
                let rtt_ns = now.duration_since(sent_at).as_nanos() as u64;
                recv_state.lock_histogram().record(rtt_ns.max(1)).ok();
                recv_state.received.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    // Reaper: pending probes older than the timeout are lost.
    let reap_state = state.clone();
    let timeout = cfg.timeout;
    let reaper = tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        loop {
            tick.tick().await;
            let now = Instant::now();
            let expired = {
                let mut pending = reap_state.lock_pending();
                let before = pending.len();
                pending.retain(|_, sent_at| now.duration_since(*sent_at) < timeout);
                (before - pending.len()) as u64
            };
            if expired > 0 {
                reap_state.lost.fetch_add(expired, Ordering::Relaxed);
            }
        }
    });

    // Paced sender.
    let payload = cfg.payload_bytes.max(packet::MIN_PAYLOAD);
    let mut buf = vec![0u8; payload];
    let total = cfg.pps * cfg.duration.as_secs().max(1);
    let mut ticker = tokio::time::interval(Duration::from_nanos(1_000_000_000 / cfg.pps.max(1)));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst);
    for seq in 0..total {
        ticker.tick().await;
        packet::encode(seq, &mut buf);
        state.lock_pending().insert(seq, Instant::now());
        if sock.send(&buf).await.is_ok() {
            state.sent.fetch_add(1, Ordering::Relaxed);
        } else {
            state.lock_pending().remove(&seq);
        }
    }

    // Grace period for in-flight echoes, then a final reap of stragglers.
    tokio::time::sleep(cfg.timeout).await;
    receiver.abort();
    reaper.abort();
    let stragglers = state.lock_pending().drain().count() as u64;
    if stragglers > 0 {
        state.lost.fetch_add(stragglers, Ordering::Relaxed);
    }

    Ok(state.summary())
}
