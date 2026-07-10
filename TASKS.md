# TASKS.md — vxedge/vxagents

## Phase 1 (critical path: exp-001 needs probes + generator)

- [x] Workspace scaffold + CI (build/test/clippy/fmt + cargo-deny, Dependabot); shared
      `vxagent-common` crate (packet codec, Prometheus text rendering, /metrics exporter)
- [x] `vxprobe`: UDP round-trip probe; p50/p95/p99 + jitter (stddev) + timeout-based loss via
      hdrhistogram; live Prometheus exporter; JSON summary; loopback e2e test in CI
      *(v1 uses userspace monotonic timestamps — see kernel-timestamp follow-up below)*
- [x] `vxtraffic`: paced generator honoring `traffic_profiles` (pps or bandwidth-derived);
      verified 49.99/50 mbit accuracy; unconnected send_to (connected UDP halves rate on
      ICMP unreachable — found in smoke test)
- [ ] Kernel timestamps (SO_TIMESTAMPING) in vxprobe; document delta vs userspace clocks
- [ ] `vxpcap`: interface capture with duration bound, writes to results dir
- [ ] Footprint budget doc + measurement (CPU%, RSS at target probe rates)
- [ ] Static musl builds for injection into minimal guests (Zephyr/FreeRTOS hosts excluded —
      those use in-image probe payloads from vxzones)

## Phase 2

- [ ] DSCP/VLAN-aware probing (verify slice classification end-to-end)
- [ ] Multi-host mode: clock-sync detection (PTP/chrony) recorded into results metadata

## Phase 3

- [ ] VSS-aligned signal emission for vehicle-manifest runs (Kuksa client feature flag)
- [ ] `vxplant`: plant-model agent for actuator/sensor closed loops — subscribes to actuator
      commands (CAN/VSS), integrates a first-order response model, publishes sensor feedback;
      driven by the manifest `actuators:` block (FMI/FMU import is phase 4)
- [ ] ARM64 release artifacts (Jetson; Snapdragon vSoC / Graviton)
