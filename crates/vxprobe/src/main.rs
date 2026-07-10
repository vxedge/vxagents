//! vxprobe — vxedge latency probe (thin CLI over the library).
//!
//! `vxprobe reflect --listen 0.0.0.0:9800`             # echo endpoint
//! `vxprobe send --target 10.0.101.2:9800 --pps 500 \
//!    --duration-s 30 --metrics 0.0.0.0:9801`          # measure + export

use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use vxprobe::{ProbeState, SendConfig, run_reflector, run_sender};

#[derive(Parser)]
#[command(
    name = "vxprobe",
    version,
    about = "vxedge latency probe (UDP round-trips)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Echo received probes back to their sender.
    Reflect {
        #[arg(long, default_value = "0.0.0.0:9800")]
        listen: SocketAddr,
    },
    /// Send paced probes at a reflector and report RTT/jitter/loss.
    Send {
        #[arg(long)]
        target: SocketAddr,
        #[arg(long, default_value_t = 100)]
        pps: u64,
        #[arg(long, default_value_t = 64)]
        payload_bytes: usize,
        #[arg(long, default_value_t = 10)]
        duration_s: u64,
        #[arg(long, default_value_t = 1000)]
        timeout_ms: u64,
        /// Serve live Prometheus metrics on this address during the run.
        #[arg(long)]
        metrics: Option<SocketAddr>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    match Cli::parse().cmd {
        Cmd::Reflect { listen } => run_reflector(listen).await?,
        Cmd::Send {
            target,
            pps,
            payload_bytes,
            duration_s,
            timeout_ms,
            metrics,
        } => {
            let state = ProbeState::new();
            if let Some(addr) = metrics {
                let s = state.clone();
                tokio::spawn(vxagent_common::exporter::serve(
                    addr,
                    Arc::new(move || s.render()),
                ));
            }
            let summary = run_sender(
                SendConfig {
                    target,
                    pps,
                    payload_bytes,
                    duration: Duration::from_secs(duration_s),
                    timeout: Duration::from_millis(timeout_ms),
                },
                state,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            if summary.received == 0 {
                // A probe that heard nothing is a failed measurement.
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
