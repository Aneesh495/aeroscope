# Aeroscope

Aeroscope is a live aviation surveillance system built around one idea: show
observed aircraft honestly, and explain the age and quality of every
observation instead of pretending the sky is fully covered.

It ingests bounded source feeds (a local `readsb` receiver or an authorized
remote provider), fuses them on an event-time timeline, serves fused tracks to
viewport subscribers over a compact versioned streaming protocol, stores
history for replay, and renders everything on a CesiumJS globe.

## Quick start

Run the backend end-to-end demonstration over synthetic replay:

```sh
make demo        # or: cargo run -p aero-server --bin aeroscope -- demo
```

It drives 2,000 globally distributed synthetic aircraft through the full
pipeline (durable log, event-time fusion, spatial index, viewport
subscription) and prints real metrics. A representative run:

```
Aeroscope demo (synthetic replay, labeled synthetic; not a live feed)
  observations accepted: 122409
  logged (durable)     : 118495
  log duplicates       : 3914
  fusion updated       : 118318
  fusion late revisions: 172
  fusion rejected      : 5
  tracks indexed       : 1992
  live tracks          : 1992
  deltas to subscriber : 59
  OK: accepted == logged + duplicates (no silent loss)
  OK: log caught all 2409 injected duplicates (+ reorder collisions)
```

Run the tests:

```sh
cargo test --workspace     # Rust backend (116 tests)
cd web && npm ci && npx vitest run   # web logic (11 tests)
```

Independently verify an acceptance bundle:

```sh
make acceptance RUN_ID=demo1          # writes evidence/demo1
make verify EVIDENCE=evidence/demo1   # recomputes and bounded-replays it
```

## What makes it different

- Event-time fusion with explicit clocks: observation, provider publish,
  ingest, fused, delivery, and render times are distinct types, and internal
  latency is reported separately from observation age.
- Honest coverage: coverage is measured from real observations and source
  metadata. A missing position never becomes a moving aircraft, and a stale or
  lost track is pinned, never animated.
- Separate identities: observation, track, flight leg, scheduled service, and
  airframe registration are modeled as different things. A callsign is not
  treated as a stable key.
- Conflicts are retained, not averaged: when sources disagree, both are kept as
  evidence and one is selected by a declared rule, with the reason recorded.
- Replay is always labeled replay and cannot masquerade as a live feed.
- An independent verifier recomputes every decisive number and rejects a
  tampered evidence bundle for a semantic reason.

## Architecture

Ten Rust crates, each owning one concern, composed by `aero-server`:

`aero-geo` (WGS84 oracle) · `aero-types` (clocks/units/identities) ·
`aero-ingest` (readsb + OpenSky adapters) · `aero-log` (durable log) ·
`aero-fusion` (event-time engine) · `aero-spatial` (viewport index) ·
`aero-stream` (protocol) · `aero-history` (partitions + replay) ·
`aero-loadgen` (synthetic load) · `aero-verify` (independent checks).

The web app (`web/`) is TypeScript/React/CesiumJS. See
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md),
[docs/SPECIFICATION.md](docs/SPECIFICATION.md),
[docs/LATENCY.md](docs/LATENCY.md),
[docs/COVERAGE.md](docs/COVERAGE.md),
[docs/RECOVERY.md](docs/RECOVERY.md),
[docs/EXPERIMENT_PLAN.md](docs/EXPERIMENT_PLAN.md), the
[ADRs](docs/adr/), and the [diagrams](docs/diagrams/).

## Status and honesty

See [docs/BUILD_STATUS.md](docs/BUILD_STATUS.md) for per-gate status. In short:
the backend core, the streaming protocol, history/replay, the globe UI logic,
and the independent verifier are implemented and tested. Resource-gated
acceptance items are reported honestly and never relabeled as passed:

- G01 live integration: blocked (needs an authorized OpenSky agreement; an API
  token is not that agreement, and replay cannot satisfy this gate).
- G04/G05/G06 capacity, latency at full load, and 20k-glyph frame traces:
  unverified (the full 50k-track, 6-hour profile and a pinned GPU host are not
  available in this environment).
- The known-truth gates (geographic correctness, event semantics, freshness
  honesty, semantic verification) pass under independent verification on the
  bounded diagnostic profile.

## License

Apache-2.0. See [docs/COVERAGE.md](docs/COVERAGE.md) and the OpenSky ADR before
enabling any remote provider; Aeroscope does not redistribute data whose
license forbids it.
