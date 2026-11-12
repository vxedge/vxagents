# Changelog

All notable changes to this repo are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning follows
[Semantic Versioning](https://semver.org/) — see `docs/VERSIONING.md` in the `docs` repo.

## [Unreleased]

## [0.2.3] - 2026-11-12

`--version` names the build profile, so vxedged can record whether a result came from a debug or a
release build. Additive: flags, series and output are otherwise unchanged.

### Added

- `--version` ends in the build profile, `(debug)` or `(release)`, as in
  `vxprobe 0.2.2 (release)`. vxedged records it with a run's results (vxedge X9.14), since a debug
  build changes what a probe measures. `debug` means a build with debug assertions on.

## [0.2.2] - 2026-11-12

`vxtraffic` records its achieved rate when a campaign stops it early. Additive: flags, series and
the summary's fields are unchanged.

### Fixed

- `vxtraffic` prints its stdout summary when stopped by SIGTERM or SIGINT, counting what it sent
  until then, and exits 0. vxedged stops a profile's load with SIGTERM at cooldown, before the
  load's own duration ends, so a campaign's traffic log was empty and the achieved rate was never
  recorded.

## [0.2.1] - 2026-11-12

`vxprobe` exports its slowest round trip, so a short measure window can be judged by it. Additive:
every existing series, flag and the stdout summary are unchanged.

### Added

- `vxprobe send --metrics` also exports `vxprobe_rtt_seconds_max`, the slowest round trip of the
  run so far, as the stdout summary's `max` reports it. A short measure window can then be judged
  by its slowest packet, which is what a cold start shows.

## [0.2.0] - 2026-11-09

`vxprobe` exports the round-trip distribution as a Prometheus histogram, so latency over time can be
drawn from successive scrapes. `vxprobe send` also keeps counting after a refused or unreachable
probe. Rust 1.99.1, and a `tokio` update for RUSTSEC-2026-0386.

**Upgrading from 0.1.x.** Additive: every existing series, flag and the stdout summary are
unchanged.

### Added

- `vxprobe send --metrics` also exports the RTT distribution as a Prometheus histogram,
  `vxprobe_rtt_distribution_seconds_bucket` (`le` from 10 µs to 10 s, 1, 2 and 5 per decade, and
  `+Inf`), `_sum` and `_count`. The counts are cumulative from the start of the run, so the
  difference between two scrapes is the distribution of the round trips in between: latency over
  time at the scrape interval, with nothing lost between scrapes. The existing series and the
  stdout summary are unchanged.

### Changed

- Lockfile update: the semver-compatible dependency updates `cargo update` offers (24 crates). No `Cargo.toml` change and no source change; build, test, clippy `-D warnings`, fmt and `cargo deny` pass.
- The toolchain moves from Rust 1.96.1 to 1.99.1 (`rust-toolchain.toml`, `rust-version` 1.99). Build, test, clippy `-D warnings`, fmt and `cargo deny` pass unchanged; no source changes were needed.

### Fixed

- `vxprobe send` stopped counting echoes after the first refused or unreachable probe, so a reflector that started late read as total loss. It now loses that probe and keeps measuring.

### Security

- Lockfile update `tokio` 1.53.1 -> 1.54.1, which fixes RUSTSEC-2026-0386 (unsound `AsyncFd` constructors). This repo does not call `AsyncFd` directly.

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
