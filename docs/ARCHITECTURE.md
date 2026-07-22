# Architecture

Aeroscope is a pipeline that turns bounded source feeds into fused aircraft
tracks, serves them to viewport subscribers, stores history for replay, and
renders them on a globe. The backend is a set of small Rust crates, each owning
one concern, composed by `aero-server`.

## Crate map

| Crate | Owns | Key invariant |
|---|---|---|
| `aero-geo` | WGS84 math, ECEF, distance, dateline | No external geodesy dep; it is the G02 oracle |
| `aero-types` | Clocks, units, identities, observation | Honesty invariants encoded as distinct types |
| `aero-ingest` | readsb + OpenSky adapters, capability registry | Explicit nulls; OpenSky disabled without an agreement |
| `aero-log` | Append-only observation log | Log before fusion; idempotent; torn-tail recovery |
| `aero-fusion` | Event-time fusion, lifecycle, plausibility, legs | Retain conflicts, never average datums |
| `aero-spatial` | Grid index, viewport queries | Dateline and poles correct; no dup/drop |
| `aero-stream` | Versioned protocol, subscriptions | Resync invariant; bounded per-client work |
| `aero-history` | Partitioned history, simplify, replay | Replay is never live; 3D error bound |
| `aero-loadgen` | Deterministic synthetic load | Reproducible; labeled synthetic |
| `aero-server` | Pipeline integration + demo binary | Durability first; metrics |
| `aero-verify` | Independent verifier + campaign | Recompute from raw, never trust booleans |

## Data path (one observation)

1. An adapter (`aero-ingest`) parses a source record, converts units, and emits
   a normalized `Observation` with explicit nulls and attribution.
2. `aero-server::Pipeline` appends it to the durable log (`aero-log`) before
   anything else. A duplicate dedup key is dropped idempotently here.
3. The observation enters `aero-fusion`. The engine runs the late-event check,
   then plausibility, then conflict selection, then advances the watermark.
4. The selected fused position updates the `aero-spatial` grid index.
5. On flush, each subscription (`aero-stream`) emits a snapshot or delta for the
   tracks in its viewport, bounded by its backpressure policy.
6. The client (`web/`) applies the message under the resync invariant and the
   globe renders it with a freshness label.

## Time model

Six wall clocks (observation, publish, ingest, fused, delivery, render) plus a
monotonic ingest clock are distinct types in `aero-types::time`. Internal
latency (ingest to delivery) is reported separately from observation age
(observation time to now). See `docs/LATENCY.md`.

## What is original here

The defining work is the event-time fusion state machine with conflict
retention, the spatial subscription protocol with its resync invariant, the
provenance-aware replay, and the independent verifier. Cesium is the globe
platform; PostGIS (in a full deployment) is durable storage. Neither replaces
the engine.
