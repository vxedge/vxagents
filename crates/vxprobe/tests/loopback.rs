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
}
