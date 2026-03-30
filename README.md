# Aeroscope

Aeroscope is a live aviation surveillance system built around one idea: show
observed aircraft honestly, and explain the age and quality of every
observation instead of pretending the sky is fully covered.

It ingests bounded source feeds (a local `readsb` receiver or an authorized
remote provider), fuses them on an event-time timeline, serves fused tracks to
viewport subscribers over a compact versioned streaming protocol, stores
history for replay, and renders everything on a CesiumJS globe.

## Status

Early, actively building. The geographic reference core (`crates/aero-geo`) is
implemented and tested. See `docs/BUILD_STATUS.md` for the per-component and
per-gate status, and `CONTRACT.md` for what this project is and is not.

## What makes it different

- Event-time fusion with explicit clocks: observation time, provider publish
  time, ingest time, fused-state time, delivery time, and render time are all
  distinct, and clock offsets are shown rather than hidden.
- Honest coverage: coverage is measured from real observations and source
  metadata. A missing position never becomes a moving aircraft.
- Separate identities: an observation, a track, a flight leg, a scheduled
  service, and an airframe registration are modeled as different things.
- Replay is always labeled replay and cannot masquerade as a live feed.

## Quick start (geographic core)

```sh
cargo test -p aero-geo
```

This exercises the WGS84 ellipsoid constants, geodetic/ECEF conversion with a
global round-trip lattice, haversine and Vincenty distance agreement, and
antimeridian splitting.

More setup, build, and demo commands land as each subsystem is wired. See
`docs/` for architecture, specification, and the experiment plan.

## License

Apache-2.0. See data and asset provenance notes in `docs/` before enabling any
remote provider; Aeroscope does not redistribute data whose license forbids it.
