# ADR 0002: Model each clock as a distinct type

## Status
Accepted.

## Context
The brief requires that internal processing latency never be confused with
observation age, and that clock offsets and missing timestamps stay explicit.
Passing `i64` milliseconds everywhere invites exactly the bug the brief forbids:
subtracting an ingest time from a delivery time and calling the result
"observation age".

## Decision
`aero-types::time` defines `ObservationTime`, `PublishTime`, `IngestWallTime`,
`FusedTime`, and `DeliveryTime` as separate newtypes, plus a `MonoNanos`
monotonic clock and a `MaybeTime<T>` for timestamps a source may omit.
`LatencyBreakdown` records observation age and internal latency as separate
fields, and `internal_ms()` returns only the part Aeroscope controls.

## Consequences
- Mixing two clocks is a compile error, not a silent logic bug.
- `MaybeTime::Absent` forces callers to decide what to do when a source omits a
  timestamp, instead of defaulting to zero or now.
- A small amount of boilerplate (a macro generates the wall-clock newtypes) in
  exchange for the guarantee.
