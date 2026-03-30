# Aeroscope project contract

Aeroscope is a low-latency aviation surveillance system: an event-time fusion
pipeline that turns bounded source feeds (a local `readsb` receiver or an
authorized remote provider) into fused aircraft tracks, serves them over a
versioned streaming protocol to viewport subscribers, stores history for
replay, and renders them on a CesiumJS globe that is honest about the age and
quality of every observation.

## What this project is

- Event-time surveillance fusion with explicit clocks and provenance.
- Aviation identity and history: observation, track, flight leg, scheduled
  service, and airframe registration are distinct entities.
- Spatial subscription delivery with bounded per-client work.
- Provenance-aware replay that never presents replay as live.
- A demanding geographic interface that explains coverage rather than inventing it.

## What this project is not

- Not a transport router, flight-dynamics simulator, or robotics stack.
- Not a universal flight tracker. Coverage is a measured property of the
  sources you configure. Missing data stays missing; it is never fabricated.
- Not a thin wrapper around a provider API or the Cesium engine. Cesium is the
  globe platform; the streaming system and flight semantics are original.

## Honesty invariants (enforced by code and the verifier)

1. A registration-country code does not establish a flight's departure country.
2. A missing position is never turned into a moving aircraft.
3. Replay is always labeled replay. It cannot satisfy the live-integration gate.
4. Barometric and geometric altitude are stored and shown separately.
5. Internal processing latency is reported separately from total observation age.
6. Unknown classification (for example charter status) stays unknown.

## Dependency and license policy

- Rust for ingestion, fusion, spatial index, streaming, and query services.
- PostgreSQL/PostGIS for durable metadata and partitioned history.
- TypeScript/React/CesiumJS for the interface; MapLibre for an optional 2D view.
- Geographic reference math (`aero-geo`) depends on no external geodesy crate so
  it can serve as the independent G02 oracle.
- OpenSky's current OAuth2 client-credentials adapter is implemented but kept
  disabled for operational use until a written agreement covering display,
  retention, and redistribution exists. An API token is not that agreement.
