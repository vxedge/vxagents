# vxagents

Lightweight Rust measurement agents for vxedge experiments: RTT/jitter/loss probes, rate-accurate traffic generation, and Prometheus exporters. They are built not to perturb what they measure. Agents are started per experiment by `vxedged`; they are not baked into guest images.

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![CI](https://github.com/vxedge/vxagents/actions/workflows/ci.yml/badge.svg)](https://github.com/vxedge/vxagents/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/tag/vxedge/vxagents?label=release)](https://github.com/vxedge/vxagents/tags)

vxedge is an open, all-software testbed for mapping 5G network slices onto zonal and central software-defined-vehicle (SDV) E/E architectures on a single machine. Every published number ships with the manifest and image digests that produced it.

## Crates

| Crate | What |
|---|---|
| `vxprobe` | Paced UDP probes plus a reflector. Reports RTT percentiles, jitter and loss, and can serve live Prometheus metrics |
| `vxtraffic` | Rate-accurate load generator backing `TrafficService` profiles (pps, bandwidth, payload) |
| `vxagent-common` | Shared timestamping, histogram and exporter helpers |

## Quickstart

```bash
cargo build --workspace
cargo test --workspace            # includes a loopback end-to-end probe test

# reflector in one terminal, sender in another
./target/debug/vxprobe reflect --help
./target/debug/vxprobe send --help
```

## What it measures

vxprobe produces every latency, jitter and loss number in
[`docs/RESULTS.md`](https://github.com/vxedge/docs/blob/main/RESULTS.md) (p50/p95/p99 per
slice, 1000 pps × 60 s per point). Timestamps are userspace-monotonic today; the move to
SO_TIMESTAMPING is tracked in [`TASKS.md`](TASKS.md).

## Links

- Site: <https://vxedge.dev>
- Architecture: [`docs/ARCHITECTURE.md`](https://github.com/vxedge/docs/blob/main/ARCHITECTURE.md)
- Results: [`docs/RESULTS.md`](https://github.com/vxedge/docs/blob/main/RESULTS.md) · Roadmap: [`docs/ROADMAP.md`](https://github.com/vxedge/docs/blob/main/ROADMAP.md)
- Contributing: [CONTRIBUTING](https://github.com/vxedge/.github/blob/main/CONTRIBUTING.md) · Security: [SECURITY](https://github.com/vxedge/.github/blob/main/SECURITY.md)
- License: Apache-2.0 ([`LICENSE`](LICENSE))
