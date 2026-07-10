//! vxtraffic core: paced UDP load matching a manifest `traffic_profiles`
//! entry (pps / bandwidth_mbps / payload_bytes). Reports target vs achieved
//! rate — the achieved figure goes into results metadata.

use serde::Serialize;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;

#[derive(Debug, Clone)]
pub struct LoadConfig {
    pub target: SocketAddr,
    pub pps: u64,
    pub payload_bytes: usize,
    pub duration: Duration,
}

/// Derive packets-per-second from a bandwidth target and payload size
/// (the manifest allows either form).
pub fn pps_for_bandwidth(bandwidth_mbps: u64, payload_bytes: usize) -> u64 {
    let bits_per_packet = (payload_bytes.max(1) * 8) as u64;
    ((bandwidth_mbps * 1_000_000) / bits_per_packet).max(1)
}

#[derive(Debug, Serialize)]
pub struct LoadSummary {
    pub sent_packets: u64,
    pub sent_bytes: u64,
    pub send_errors: u64,
    pub elapsed_s: f64,
    pub target_pps: u64,
    pub achieved_pps: f64,
    pub achieved_mbps: f64,
}

/// Blast paced UDP datagrams; returns achieved-rate summary.
pub async fn run_load(cfg: LoadConfig) -> std::io::Result<LoadSummary> {
    // Unconnected socket + send_to on purpose: a connected UDP socket has
    // ICMP port-unreachable errors delivered on subsequent send() calls,
    // silently halving the achieved rate against a sink with no listener.
    // Load generation is fire-and-forget.
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    let buf = vec![0u8; cfg.payload_bytes.max(1)];
    let total = cfg.pps * cfg.duration.as_secs().max(1);
    let mut ticker = tokio::time::interval(Duration::from_nanos(1_000_000_000 / cfg.pps.max(1)));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst);

    let started = Instant::now();
    let mut sent_packets = 0u64;
    let mut sent_bytes = 0u64;
    let mut send_errors = 0u64;
    for _ in 0..total {
        ticker.tick().await;
        match sock.send_to(&buf, cfg.target).await {
            Ok(n) => {
                sent_packets += 1;
                sent_bytes += n as u64;
            }
            Err(_) => send_errors += 1,
        }
    }
    let elapsed = started.elapsed().as_secs_f64().max(f64::EPSILON);
    Ok(LoadSummary {
        sent_packets,
        sent_bytes,
        send_errors,
        elapsed_s: elapsed,
        target_pps: cfg.pps,
        achieved_pps: sent_packets as f64 / elapsed,
        achieved_mbps: (sent_bytes as f64 * 8.0) / elapsed / 1e6,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bandwidth_to_pps() {
        // 400 mbit at 1200-byte payloads = 400e6 / 9600 bits.
        assert_eq!(pps_for_bandwidth(400, 1200), 41_666);
        // Floors at 1 pps.
        assert_eq!(pps_for_bandwidth(0, 1200), 1);
        // Zero payload guarded.
        assert!(pps_for_bandwidth(10, 0) > 0);
    }
}
