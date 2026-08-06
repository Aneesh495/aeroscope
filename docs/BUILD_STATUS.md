# Build status

Last updated: backend core, streaming, history, UI logic, and verifier
implemented and tested; resource-gated gates honest.

## Component status

| Component | Crate / path | Status | Notes |
|---|---|---|---|
| Geographic reference oracle | `crates/aero-geo` | done, tested | WGS84/ECEF/haversine/vincenty/dateline |
| Core types and units | `crates/aero-types` | done, tested | distinct clocks, altitude datums, identities |
| Source adapters | `crates/aero-ingest` | done, tested | readsb JSON + OpenSky OAuth2 (disabled) + registry |
| Observation log | `crates/aero-log` | done, tested | append-only, CRC, recovery, idempotent |
| Event-time fusion | `crates/aero-fusion` | done, tested | watermark, lifecycle, plausibility, legs |
| Spatial index | `crates/aero-spatial` | done, tested | viewport queries, dateline, poles |
| Streaming protocol | `crates/aero-stream` | done, tested | snapshot/delta, backpressure, resync |
| History + replay | `crates/aero-history` | done, tested | partitions, 3D path simplify, replay controller |
| Load generator | `crates/aero-loadgen` | done, tested | deterministic, global, seeded faults |
| Server + demo | `crates/aero-server` | done, tested | integrated pipeline, metrics, demo binary |
| Verifier + campaign | `crates/aero-verify` | done, tested | independent recompute + bounded replay |
| Web UI logic | `web/src` | done, tested | protocol client, freshness, coverage, components |
| Network transport | (server) | planned | async HTTP/WebSocket layer to connect UI to pipeline |
| PostGIS persistence | (history) | planned | in-memory partitions mirror the intended schema |

## Test totals

- Rust: 116 tests across the workspace, all green.
- Web: 11 vitest tests, typecheck clean.
- Demo binary runs end to end and asserts no silent loss.
- Campaign -> verify roundtrip works; tampered bundles are rejected.

## Gate status (honest)

| Gate | Status | Reason |
|---|---|---|
| G01 live integration | blocked | no authorized OpenSky agreement in this environment; replay cannot satisfy |
| G02 geographic correctness | passed (diagnostic) | independent formulation agrees to <1e-3 m ECEF, <1e-6 relative distance |
| G03 event semantics | passed (diagnostic) | seeded dup/late/reject counts reproduce exactly under bounded replay |
| G04 streaming capacity (6h/50k) | unverified | full profile needs a declared reference host; pilot only |
| G05 internal latency | unverified | measured on bounded pilot, not the full host at full load |
| G06 frontend performance | unverified | no pinned-GPU 20k-glyph frame trace available here |
| G07 freshness honesty | passed (diagnostic) | stale/unknown states driven by declared lifecycle horizons |
| G08 recovery | partial | torn-tail recovery and client resync tested; full kill/restart harness pending |
| G09 historical | partial | range queries, simplify, replay tested; full reconciliation harness pending |
| G10 provider limits | partial | capability registry + OpenSky retry/quota logic tested; live quota logs pending |
| G11 UI quality | partial | component + logic tests; real browser traces/screenshots pending a GPU host |
| G12 source coverage | passed | coverage computed from observations; capability table published |
| G13 semantic verifier | passed | independent recompute rejects tampered bundles with nonzero exit |
| G14 release | partial | repo published, CI added, clean build; full campaign gates outstanding |

Status words follow the brief. Nothing resource-gated is relabeled passed.
"Diagnostic" means verified on the bounded pilot profile, not the full host.
