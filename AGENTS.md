# AGENTS.md — vxedge/vxagents

Rust measurement agents deployed per experiment into zones, compute nodes, and the gateway
path. Master spec: `../docs/ARCHITECTURE.md` §11 (agents), §12 (metrics: p50/p95/p99 latency,
jitter, packet loss per slice).

## What lands here

- Latency/jitter probe agents (timestamped UDP/TCP probes between zone↔compute pairs).
- Traffic generator agent (backs `TrafficService` synthetic profiles: pps, bandwidth, payload).
- PCAP capture control (per-interface, duration-bounded, per §6 `telemetry.pcap`).
- Prometheus exporters for everything above.

## Rules

- **Lightweight is a requirement, not a preference**: agents must not perturb the experiment
  (§11). Budget and document CPU/memory per agent; no allocations on the hot probe path;
  prefer `SO_TIMESTAMPING`/kernel timestamps over userspace clocks.
- Agents are injected/started per experiment by vxedged — never baked into zone images and
  never long-running daemons outside an experiment.
- Report timestamps monotonic + wall-clock; document clock-sync assumptions (single host:
  shared clock; multi-host: PTP/chrony noted in results metadata).
- Rust only (§15); binaries must build for x86-64 and ARM64 (phase-4 Jetson).

## Project skills

AI/contributor skills in `.claude/skills/`: **rust-patterns** (house Rust rules, per the
Microsoft Rust Patterns book) and **testbed-quality** (the OEM-adoption bar). Apply both to
every change.
