# TASKS.md — vxedge/vxagents

## Phase 1 (critical path: exp-001 needs probes + generator)

- [ ] Workspace scaffold + CI; shared `vxagent-common` crate (metrics, config, logging)
- [ ] `vxprobe`: UDP round-trip probe with kernel timestamps; p50/p95/p99 latency + jitter +
      loss computed client-side; Prometheus exporter
- [ ] `vxtraffic`: synthetic generator honoring `traffic_profiles` (pps, bandwidth_mbps,
      payload_bytes); rate accuracy test
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
