# Changelog

All notable changes to this repo are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
[Semantic Versioning](https://semver.org/) — see `docs/VERSIONING.md` in the `docs` repo.

## [Unreleased]

## [0.1.1] - 2026-10-22

### Added

- Tag-triggered release workflow (`.github/workflows/release.yml`): builds `vxprobe` and `vxtraffic` for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` (native Ubuntu 24.04 runners, `--locked`), publishes `vxagents-<version>-<target>.tar.gz` archives with a `SHA256SUMS`, and attaches build provenance (`gh attestation verify`). `workflow_dispatch` is a dry run that creates no release.
- Open-source release preparation: `LICENSE` (Apache-2.0), `NOTICE`, rewritten `README.md`. Crate `license` metadata is now `Apache-2.0` (was `MIT OR Apache-2.0`).

## [0.1.0] - 2026-07-13

**Phase-1 gate passed**: H1, H3, and H4 all have reproducible numbers (`vxperiments`).
Repositories were still private at this release.

### Added
- `vxagent-common`: shared probe-packet encoding, minimal Prometheus text rendering, and a
  minimal HTTP `/metrics` exporter (2026-07-10).
- `vxprobe`: UDP round-trip latency probe — RTT p50/p95/p99, jitter (stddev), loss, live
  Prometheus export; loopback e2e test in CI (2026-07-10).
- `vxtraffic`: rate-accurate synthetic load generator; found and fixed a connected-UDP
  socket half-rate bug during development (2026-07-10).
- CI: build/test/clippy/fmt (incl. loopback e2e), Dependabot, `cargo-deny`.

### Known gaps
- `vxpcap` (per-interface packet capture) and kernel timestamps (`SO_TIMESTAMPING`, vs.
  today's userspace `Instant`-based timestamping) remain open — tracked in `TASKS.md`.
