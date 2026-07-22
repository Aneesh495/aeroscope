# Experiment plan

This plan is frozen before final measurements. It declares the workloads,
evaluation units, tolerances, seeds, and decision rules, and separates the CPU
diagnostic profile from the full acceptance profile.

## Profiles

- Diagnostic (this environment): a bounded pilot. Synthetic fleet of 2,000
  aircraft, 60 ticks at 1 Hz, seed 42, duplicate probability 0.02, reorder
  probability 0.01. Single-threaded. Runs in a few seconds.
- Full acceptance (declared reference host, not this environment): 50,000 active
  tracks, 10,000 updates/second, 200 concurrent viewport subscribers, six hours,
  seeded faults, bounded resources.

A diagnostic run can demonstrate correctness (known-truth) but cannot certify
the capacity, latency, or frontend gates. Those stay unverified until executed
on the declared host.

## Engineering hypotheses (predeclared)

1. Delta encoding with per-track coalescing reduces bytes to a slow consumer
   without breaking state consistency (measured: byte count and a verifier
   consistency check after coalescing).
2. The fusion watermark keeps internal latency bounded when one source lags
   (measured: p95 internal latency with and without an injected slow source).
3. Adaptive trail decimation improves frame time within a fixed geographic error
   bound (measured: p95 frame time and max path deviation in metres).
4. Idempotent logging catches every injected duplicate (measured: log
   duplicates >= injected duplicates; verified independently).
5. Plausibility screening rejects seeded impossible jumps without rejecting
   genuine maneuvers (measured: rejection count vs injected-jump count).
6. The grid index query cost scales with viewport area, not fleet size
   (measured: query time across viewport sizes at fixed fleet).

## Evaluation units and tolerances

- Geographic (G02): >= 100,000 independent conversion/distance cases; ECEF
  agreement < 1e-3 m, relative distance agreement < 1e-6.
- Event semantics (G03): exact independent counts of updated/late/rejected/
  duplicate must match a bounded replay.
- Internal latency (G05): p95 < 150 ms, p99 < 500 ms at the declared load.
- Frontend (G06): >= 30 FPS p95 at 1440p with 20,000 glyphs.

## Decision rules

A threshold is never lowered, a workload never shrunk, and a poor seed never
dropped after seeing a result. A protocol change requires a new version and a
fresh campaign that retains the earlier results.

## Status in this session

The diagnostic profile runs and the known-truth gates (G02, G03, G07, G13) pass
under independent verification. The full-host gates (G01, G04, G05, G06) are
blocked or unverified for lack of an authorized live feed and a declared
reference host. These are reported honestly, not substituted.
