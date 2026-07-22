# Source coverage and capability table (G12)

Coverage in Aeroscope is a measured property of the sources you configure, not
a claim. This page distinguishes three different things the brief insists must
never be blurred:

1. Supported traffic categories: what the system can represent when the data
   identifies it (international, domestic, regional, cargo, general aviation,
   charter).
2. Known observed instances: aircraft actually seen by a configured source in a
   given region and time window.
3. Unknown classifications: things the data does not establish (for example,
   whether a flight is a charter), which remain unknown rather than guessed.

## Capability table

Values come from `aero_ingest::CapabilityRegistry` defaults. "Global" scope
means the source does not restrict queries by region; it does not mean global
coverage.

| Source | Scope | Cadence | Quota | Retention | Callsign | Baro alt | Geo alt | Route | Redistribution |
|---|---|---|---|---|---|---|---|---|---|
| readsb (owned receiver) | receiver range (km) | ~1 s | none | operator's own | yes | yes | yes | no | operator's own |
| OpenSky (REST) | global query scope | ~5 s | 4000 / day | none without agreement | yes | yes | yes | no | disabled (needs agreement) |

## Supported vs observed

- Supported: all six traffic categories are representable; the identity model
  (observation, track, leg, schedule, registration) keeps them distinct.
- Observed: computed at runtime by `web/src/coverage.ts` from real positions,
  binned into a coarse grid, reported as sample density with an explicit note
  that density is not an absolute aircraft count and says nothing about
  unsampled regions.
- Unknown: charter status and similar classifications are modeled with
  `Provenance::Absent` and shown as unknown.

## How to measure your own coverage

Run a source for a window, then query the coverage summary. The result lists
observed cells and total observations with the density disclaimer. It will show
gaps (oceanic regions, areas outside a receiver's range) as gaps, because those
are genuinely unobserved, not empty of aircraft.

## Honesty note

A registration-country code does not establish a flight's departure country.
Aeroscope stores registration, operator, type, callsign, route inference, and
schedule as separate identities, each with its own provenance, so one is never
silently used as evidence for another.
