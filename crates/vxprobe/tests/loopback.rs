//! End-to-end loopback test: reflector + sender over 127.0.0.1. Runs
//! unprivileged, so it executes in plain `cargo test` and CI.

use std::time::Duration;
use tokio::net::UdpSocket;
use vxprobe::{ProbeState, SendConfig, run_reflector, run_sender};

#[tokio::test(flavor = "multi_thread")]
async fn probe_measures_over_loopback() {
    // Grab an ephemeral port, release it, and reflect there.
    let addr = {
        let s = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        s.local_addr().unwrap()
    };
    tokio::spawn(run_reflector(addr));
    tokio::time::sleep(Duration::from_millis(50)).await;

    let state = ProbeState::new();
    let summary = run_sender(
        SendConfig {
            target: addr,
            pps: 200,
            payload_bytes: 64,
            duration: Duration::from_secs(1),
            timeout: Duration::from_millis(500),
        },
        state.clone(),
    )
    .await
    .expect("sender runs");

    assert_eq!(
        summary.sent, 200,
        "paced sender emits pps * duration probes"
    );
    assert!(
        summary.received >= 190,
        "loopback should deliver nearly everything, got {}",
        summary.received
    );
    assert!(summary.rtt_us.p50 > 0.0, "p50 measured");
    assert!(
        summary.rtt_us.p99 >= summary.rtt_us.p50,
        "percentiles ordered"
    );

    // Live Prometheus rendering carries the same counters.
    let rendered = state.render();
    assert!(rendered.contains("vxprobe_sent_total 200"));
    assert!(rendered.contains("vxprobe_rtt_seconds{quantile=\"0.99\"}"));
    // The slowest round trip so far, as the summary's `max` reports it.
    let max = rendered
        .lines()
        .find_map(|l| l.strip_prefix("vxprobe_rtt_seconds_max "))
        .and_then(|v| v.parse::<f64>().ok())
        .expect("vxprobe_rtt_seconds_max is exported");
    assert!(
        (max * 1e6 - summary.rtt_us.max).abs() < 1.0,
        "{max} s vs {} us",
        summary.rtt_us.max
    );
}

/// A probe sent before the reflector is up comes back as a refused port. That is one lost probe,
/// not the end of the run: the receiver used to exit on it, and every later echo read as loss.
#[tokio::test]
async fn a_reflector_that_starts_late_is_still_measured() {
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = sock.local_addr().unwrap();
    drop(sock);
    let state = ProbeState::new();
    let cfg = SendConfig {
        target: addr,
        pps: 50,
        payload_bytes: 64,
        duration: Duration::from_secs(2),
        timeout: Duration::from_millis(200),
    };
    let sender = tokio::spawn(run_sender(cfg, state));

    tokio::time::sleep(Duration::from_millis(500)).await;
    tokio::spawn(run_reflector(addr));

    let summary = sender.await.unwrap().unwrap();
    assert!(summary.received > 0, "no echoes counted: {summary:?}");
}
