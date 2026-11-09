//! vxprobe core: a reflector that echoes UDP packets, and a sender that
//! paces sequenced probes, correlates echoes, and reports RTT percentiles,
//! jitter, and loss.
//!
//! Timestamping: monotonic userspace clock (`Instant`) around send/receive.
//! Documented limitation: kernel timestamps (SO_TIMESTAMPING) are not
//! supported. Userspace clocks add scheduler noise to the tails,
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

/// Upper bounds of the RTT distribution's buckets, in seconds, as Prometheus `le` labels: 1, 2
/// and 5 per decade from 10 µs to 10 s. The counts are cumulative from the start of the run, so
/// the difference between two scrapes is the distribution of the RTTs in between: latency over
/// time, at the scrape interval, with nothing lost between scrapes.
pub const RTT_BUCKETS: [(&str, u64); 19] = [
    ("0.00001", 10_000),
    ("0.00002", 20_000),
    ("0.00005", 50_000),
    ("0.0001", 100_000),
    ("0.0002", 200_000),
    ("0.0005", 500_000),
    ("0.001", 1_000_000),
    ("0.002", 2_000_000),
    ("0.005", 5_000_000),
    ("0.01", 10_000_000),
    ("0.02", 20_000_000),
    ("0.05", 50_000_000),
    ("0.1", 100_000_000),
    ("0.2", 200_000_000),
    ("0.5", 500_000_000),
    ("1", 1_000_000_000),
    ("2", 2_000_000_000),
    ("5", 5_000_000_000),
    ("10", 10_000_000_000),
];

/// Shared live state, rendered by the Prometheus exporter during a run.
/// std::sync mutexes on purpose: critical sections are tiny and lock-free
/// counters carry the hot-path totals.
pub struct ProbeState {
    sent: AtomicU64,
    received: AtomicU64,
    lost: AtomicU64,
    // RTTs recorded in nanoseconds, 3 significant figures, up to 60 s.
    histogram: Mutex<Histogram<u64>>,
    /// RTTs per bucket of [`RTT_BUCKETS`], not cumulative; the last slot is above the top bound.
    buckets: [AtomicU64; RTT_BUCKETS.len() + 1],
    rtt_sum_ns: AtomicU64,
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
            buckets: std::array::from_fn(|_| AtomicU64::new(0)),
            rtt_sum_ns: AtomicU64::new(0),
            pending: Mutex::new(HashMap::new()),
        })
    }

    fn lock_histogram(&self) -> std::sync::MutexGuard<'_, Histogram<u64>> {
        self.histogram.lock().expect("histogram lock poisoned")
    }

    /// Record one round trip in the summary histogram and the exported distribution.
    pub fn record_rtt(&self, rtt_ns: u64) {
        let rtt_ns = rtt_ns.max(1);
        self.lock_histogram().record(rtt_ns).ok();
        let slot = RTT_BUCKETS
            .iter()
            .position(|(_, le)| rtt_ns <= *le)
            .unwrap_or(RTT_BUCKETS.len());
        self.buckets[slot].fetch_add(1, Ordering::Relaxed);
        self.rtt_sum_ns.fetch_add(rtt_ns, Ordering::Relaxed);
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
        drop(h);
        let mut cumulative = 0u64;
        for (i, (le, _)) in RTT_BUCKETS.iter().enumerate() {
            cumulative += self.buckets[i].load(Ordering::Relaxed);
            prom::line(
                &mut out,
                "vxprobe_rtt_distribution_seconds_bucket",
                &[("le", le)],
                cumulative as f64,
            );
        }
        cumulative += self.buckets[RTT_BUCKETS.len()].load(Ordering::Relaxed);
        prom::line(
            &mut out,
            "vxprobe_rtt_distribution_seconds_bucket",
            &[("le", "+Inf")],
            cumulative as f64,
        );
        prom::line(
            &mut out,
            "vxprobe_rtt_distribution_seconds_sum",
            &[],
            self.rtt_sum_ns.load(Ordering::Relaxed) as f64 / 1e9,
        );
        prom::line(
            &mut out,
            "vxprobe_rtt_distribution_seconds_count",
            &[],
            cumulative as f64,
        );
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

fn is_transient(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::{ConnectionRefused, HostUnreachable, NetworkUnreachable};
    matches!(
        e.kind(),
        ConnectionRefused | HostUnreachable | NetworkUnreachable
    )
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
            let n = match recv_sock.recv(&mut buf).await {
                Ok(n) => n,
                // A connected socket reports the ICMP answer to one probe here. That probe is
                // lost, which the reaper counts, and the next one can still get through.
                Err(e) if is_transient(&e) => continue,
                Err(_) => return,
            };
            let now = Instant::now();
            if let Some(seq) = packet::decode(&buf[..n])
                && let Some(sent_at) = recv_state.lock_pending().remove(&seq)
            {
                let rtt_ns = now.duration_since(sent_at).as_nanos() as u64;
                recv_state.record_rtt(rtt_ns);
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

#[cfg(test)]
mod distribution_tests {
    use super::*;

    fn value(text: &str, series: &str) -> f64 {
        text.lines()
            .find_map(|l| l.strip_prefix(series)?.trim().parse().ok())
            .unwrap_or_else(|| panic!("no `{series}` in\n{text}"))
    }

    /// Buckets are cumulative, `+Inf` and `_count` agree, and `_sum` is in seconds.
    #[test]
    fn round_trips_land_in_cumulative_buckets() {
        let state = ProbeState::new();
        for ns in [5_000, 15_000, 40_000, 3_000_000, 20_000_000_000] {
            state.record_rtt(ns);
        }
        let text = state.render();
        let bucket = |le: &str| {
            value(
                &text,
                &format!("vxprobe_rtt_distribution_seconds_bucket{{le=\"{le}\"}}"),
            )
        };
        assert_eq!(bucket("0.00001"), 1.0);
        assert_eq!(bucket("0.00002"), 2.0);
        assert_eq!(bucket("0.00005"), 3.0);
        assert_eq!(bucket("0.005"), 4.0);
        assert_eq!(bucket("10"), 4.0);
        assert_eq!(bucket("+Inf"), 5.0);
        assert_eq!(value(&text, "vxprobe_rtt_distribution_seconds_count"), 5.0);
        let sum = value(&text, "vxprobe_rtt_distribution_seconds_sum");
        assert!((sum - 20.00306).abs() < 1e-9, "{sum}");
    }

    /// Before any round trip, every bucket reads zero and the quantiles are left out.
    #[test]
    fn an_idle_probe_exports_empty_buckets() {
        let text = ProbeState::new().render();
        assert_eq!(
            value(
                &text,
                "vxprobe_rtt_distribution_seconds_bucket{le=\"+Inf\"}"
            ),
            0.0
        );
        assert!(!text.contains("quantile"));
    }
}
