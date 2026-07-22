# ADR 0001: An in-crate geographic oracle with no external geodesy dependency

## Status
Accepted.

## Context
Gate G02 requires validating the system's geographic math against an
independent implementation to a metre-level tolerance across 100,000+ cases.
If the system and the oracle both come from the same geodesy crate, agreement
proves nothing: a bug in the crate would appear in both.

## Decision
`aero-geo` implements WGS84 constants, geodetic/ECEF conversion (Bowring),
haversine and Vincenty distance, and antimeridian handling from first
principles with no external geodesy dependency. The verifier (`aero-verify`)
then carries a *second*, deliberately different formulation (spherical law of
cosines, an independent ECEF transcription) so the G02 comparison is a genuine
differential test.

## Consequences
- We own and must maintain the math, including the Vincenty non-convergence
  case near antipodes (handled by returning `None`).
- Agreement between two independent formulations to sub-metre ECEF and <1e-6
  relative distance gives real confidence, not a tautology.
- Derived constants (b, e^2) are computed from the two defining constants so a
  transcription error cannot desynchronize them; a test pins them to published
  values.
