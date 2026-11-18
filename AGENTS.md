# AGENTS.md — vxedge/vxagents

Rust measurement agents deployed per experiment into zones, compute nodes, and the gateway
path. Master spec: `../docs/ARCHITECTURE.md` §11 (agents), §12 (metrics: p50/p95/p99 latency,
jitter, packet loss per slice).

## What is here

- `vxprobe`: timestamped UDP round-trip probes and a reflector (RTT percentiles, jitter,
  loss), with a live Prometheus exporter.
- `vxtraffic`: traffic generator backing `TrafficService` synthetic profiles (pps,
  bandwidth, payload).
- `vxagent-common`: shared packet codec, Prometheus text rendering and `/metrics` exporter.

## Rules

- **Branch names use the `vx/` prefix**, for example `vx/fix-probe-drift`. Never name a branch after a
  tool or vendor. Delete a branch once its PR is merged.
- **Merge only after formal checks.** Before a PR is merged: its CI is green on the PR head; `cargo build --workspace`,
  `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`
  and `cargo deny check`
  ran clean locally; and the diff was re-read for claims beyond `docs/FACTS.md`, host details and
  personal or private material. A failing check blocks the merge; never skip, weaken or disable one.
- **Describe vxedge with `project.summary` from `docs/FACTS.md`** (the README's first lines quote it).
  Do not write another description of vxedge; 5G slicing is one family of its measured cases.
- **Lightweight is a requirement, not a preference**: agents must not perturb the experiment
  (§11). Budget and document CPU/memory per agent; no allocations on the hot probe path;
  prefer `SO_TIMESTAMPING`/kernel timestamps over userspace clocks.
- Agents are injected/started per experiment by vxedged — never baked into zone images and
  never long-running daemons outside an experiment.
- Report timestamps monotonic + wall-clock; document clock-sync assumptions (single host:
  shared clock; multi-host: PTP/chrony noted in results metadata).
- Rust only (§15); binaries must build for x86-64 and ARM64.

## Project skills

AI/contributor skills in `.claude/skills/`: **rust-patterns** (house Rust rules, per the
Microsoft Rust Patterns book) and **testbed-quality** (the OEM-adoption bar). Apply both to
every change.

## Work tracking

Work items are tracked on the maintainers' internal board; the public record of changes is
`CHANGELOG.md`.
