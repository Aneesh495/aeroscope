# Build status

Last updated: phase 2 (foundational representations).

## Component status

| Component | Crate / path | Status | Notes |
|---|---|---|---|
| Geographic reference oracle | `crates/aero-geo` | building, tested | WGS84/ECEF/haversine/vincenty/dateline, 11 tests green |
| Core types and units | `crates/aero-types` | planned | observation, track, identity, clocks |
| Source adapters | `crates/aero-ingest` | planned | readsb JSON + OpenSky OAuth2 (disabled) |
| Observation log | `crates/aero-log` | planned | append-only, checksummed, recoverable |
| Event-time fusion | `crates/aero-fusion` | planned | watermark state machine, track lifecycle |
| Spatial index | `crates/aero-spatial` | planned | viewport subscriptions, dateline/poles |
| Streaming protocol | `crates/aero-stream` | planned | snapshot/delta, backpressure, resync |
| Load generator | `crates/aero-loadgen` | planned | global movement, seeded faults |
| Server | `crates/aero-server` | planned | wires the pipeline, health/readiness |
| Verifier | `crates/aero-verify` | planned | independent recompute + bounded replay |
| Web UI | `web/` | planned | Cesium globe, detail drawer, timeline |

## Gate status (honest)

| Gate | Status | Reason |
|---|---|---|
| G01 live integration | blocked | no authorized OpenSky agreement available in this environment |
| G02 geographic correctness | in progress | oracle built; 100k-case campaign pending |
| G03 event semantics | planned | |
| G04 streaming capacity (6h/50k) | unverified | full campaign not executed in session |
| G05 internal latency | planned | short bounded runs only |
| G06 frontend performance | unverified | no pinned GPU trace available in session |
| G07 freshness honesty | planned | |
| G08 recovery | planned | |
| G09 historical | planned | |
| G10 provider limits | planned | mocked quota server |
| G11 UI quality | planned | |
| G12 source coverage | planned | |
| G13 semantic verifier | planned | |
| G14 release | partial | repo init + skeleton done |

Status words follow the brief: `passed`, `failed`, `blocked`, `unverified`,
and planned/in-progress for work not yet reaching a gate. Nothing resource-gated
is ever relabeled passed.
