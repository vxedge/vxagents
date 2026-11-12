//! vxtraffic — vxedge synthetic traffic generator (thin CLI).
//!
//! `vxtraffic --target 10.0.103.2:9900 --bandwidth-mbps 400 --duration-s 30`
//! `vxtraffic --target 10.0.103.2:9900 --pps 10000 --payload-bytes 1200 --duration-s 30`

use clap::Parser;
use std::net::SocketAddr;
use std::time::Duration;
use vxtraffic::{LoadConfig, pps_for_bandwidth, run_load_until};

/// `--version`: the version, then the build profile, which vxedged records with a run's results.
/// `debug` is a build with debug assertions on.
const VERSION: &str = if cfg!(debug_assertions) {
    concat!(env!("CARGO_PKG_VERSION"), " (debug)")
} else {
    concat!(env!("CARGO_PKG_VERSION"), " (release)")
};

#[derive(Parser)]
#[command(
    name = "vxtraffic",
    version = VERSION,
    about = "vxedge synthetic UDP load generator"
)]
struct Cli {
    #[arg(long)]
    target: SocketAddr,
    /// Explicit packet rate; mutually exclusive with --bandwidth-mbps.
    #[arg(long, conflicts_with = "bandwidth_mbps")]
    pps: Option<u64>,
    /// Bandwidth target; pps derived from payload size.
    #[arg(long)]
    bandwidth_mbps: Option<u64>,
    #[arg(long, default_value_t = 1200)]
    payload_bytes: usize,
    #[arg(long, default_value_t = 10)]
    duration_s: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr) // stdout is the machine-readable JSON contract
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let pps = match (cli.pps, cli.bandwidth_mbps) {
        (Some(pps), _) => pps,
        (None, Some(mbps)) => pps_for_bandwidth(mbps, cli.payload_bytes),
        (None, None) => return Err("pass --pps or --bandwidth-mbps".into()),
    };
    // A run stopped by SIGTERM or SIGINT still prints its summary: vxedged stops a profile's load
    // with SIGTERM at cooldown, and the summary is the only record of the achieved rate.
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let stop = async move {
        tokio::select! {
            _ = term.recv() => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    };
    let summary = run_load_until(
        LoadConfig {
            target: cli.target,
            pps,
            payload_bytes: cli.payload_bytes,
            duration: Duration::from_secs(cli.duration_s),
        },
        stop,
    )
    .await?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
