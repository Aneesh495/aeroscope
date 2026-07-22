# Specification and numerical contracts

This document pins the units, frames, and semantics the code relies on. The
types in `aero-types` and `aero-geo` enforce most of these; this page is the
prose contract behind them.

## Coordinate frames

- Geodetic: WGS84 latitude/longitude in degrees. Latitude in [-90, 90],
  longitude normalized to [-180, 180) with +180 mapping to -180.
- ECEF: WGS84 earth-centred earth-fixed, metres. Used for 3D distances and
  aircraft placement.
- Altitude for ECEF placement is geometric height above the WGS84 ellipsoid.

## Units

| Quantity | Stored unit | Type |
|---|---|---|
| Barometric altitude | metres (pressure altitude, 1013.25 hPa datum) | `BaroAltM` |
| Geometric altitude | metres above WGS84 ellipsoid | `GeoAltM` |
| Ground speed | metres per second | `SpeedMps` |
| Vertical rate | metres per second, positive up | `VerticalRateMps` |
| True track | degrees [0, 360) | `TrueTrackDeg` |
| Heading | degrees [0, 360) | `HeadingDeg` |

Barometric and geometric altitude are distinct types with no cross conversion,
because they are different physical quantities.

## Time

See `docs/LATENCY.md`. Six wall clocks and one monotonic clock, all distinct
types. `MaybeTime<T>` represents a timestamp a source may omit;
`EventTimeBasis` records which clock an effective event time came from.

## Event-time semantics

- Watermark: advances monotonically with the maximum accepted event time.
- Allowed lateness: observations within `watermark - allowed_lateness` fuse
  normally; older ones are late revisions that update history but not the live
  watermark.
- Deduplication key: (source, icao24, effective event time, event-time basis).
- Plausibility: impossible jump (> 1000 m/s implied), time reversal, invalid
  coordinates, ground/air contradiction. A failing check retains the evidence
  and does not move the position.

## Identity

Observation, track, flight leg, scheduled service, and airframe registration
are distinct entities. ICAO24 identifies a transponder (usually one airframe,
not guaranteed). A callsign is neither unique across a day nor a stable key.
Enrichments carry `Provenance` (authoritative, inferred, stale, absent).

## Streaming

Protocol version 1. Snapshots carry a snapshot id and base sequence; deltas
name the snapshot id they build on. The resync invariant: a delta is applied
only on top of its named snapshot and only if its sequence is exactly one more
than the last. Any violation forces a fresh snapshot.
