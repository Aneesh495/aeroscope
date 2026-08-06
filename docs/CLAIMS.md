# Claim-to-evidence map

Every behavioral or performance claim Aeroscope makes should trace to code, a
test, and an observed result. This table is the index.

| Claim | Evidence (code) | Evidence (test/run) | Status |
|---|---|---|---|
| Geodetic/ECEF round-trips to sub-mm | `aero-geo/ecef.rs` | `ecef::tests::roundtrip_many` | passed |
| Two independent geodesy formulations agree | `aero-verify/independent_geo.rs` | `distance_agrees_*`, `ecef_agrees_*` | passed |
| Dateline tracks are not dropped or duplicated | `aero-spatial/viewport.rs`, `grid.rs` | `dateline_query_finds_both_sides` | passed |
| Clocks cannot be confused at compile time | `aero-types/time.rs` | `clocks_are_distinct_types_*` | passed |
| Internal latency is separate from observation age | `aero-types/time.rs` | `LatencyBreakdown::internal_ms` + `docs/LATENCY.md` | passed |
| Barometric and geometric altitude never averaged | `aero-types/units.rs` | `baro_and_geo_are_distinct_types` | passed |
| Malformed records are quarantined, not dropped | `aero-ingest/readsb.rs` | `parses_valid_aircraft_and_quarantines_bad_hex` | passed |
| OpenSky disabled without an agreement | `aero-ingest/opensky.rs` | `disabled_guard_blocks_without_network` | passed |
| Log recovers from a torn tail | `aero-log/lib.rs` | `torn_tail_is_truncated_on_recovery` | passed |
| Ingestion is idempotent | `aero-log/lib.rs`, `aero-server/pipeline.rs` | `idempotent_append_*`, `duplicate_is_idempotent_through_pipeline` | passed |
| Conflicts retained, selected not averaged | `aero-fusion/engine.rs` | `higher_trust_source_wins_conflict_without_averaging` | passed |
| Impossible jumps rejected, evidence kept | `aero-fusion/plausibility.rs`, `engine.rs` | `impossible_jump_rejected_but_evidence_retained` | passed |
| Late events do not move the watermark back | `aero-fusion/engine.rs` | `late_event_recorded_not_applied_to_watermark` | passed |
| Extrapolation expires at a declared horizon | `aero-fusion/lifecycle.rs` | `transitions_follow_declared_horizons` | passed |
| Flight legs split on evidence, not callsign alone | `aero-fusion/segmentation.rs` | `callsign_change_splits_with_lower_confidence` | passed |
| Old delta cannot apply to a newer snapshot | `aero-stream/subscription.rs`, `web/protocol.ts` | `resync_invariant_*`, `rejects a delta on a stale snapshot` | passed |
| Slow consumer coalesced then resynced, never blocks | `aero-stream/subscription.rs` | `overflowing_hard_cap_forces_resync_not_blocking` | passed |
| Replay is never live | `aero-history/replay.rs` | `replay_is_never_live` | passed |
| Path simplification respects a 3D metre bound | `aero-history/simplify.rs` | `deviation_respects_tolerance` | passed |
| No silent loss through the pipeline | `aero-server/pipeline.rs` | `full_pipeline_no_silent_loss` | passed |
| Verifier rejects tampered evidence | `aero-verify/lib.rs` | `tampered_counts_are_rejected` + CI negative control | passed |
| p95 internal latency < 150 ms at full load | `aero-fusion`, `aero-stream` | full campaign | unverified (no reference host) |
| 30 FPS at 20k glyphs, 1440p | `web/GlobeView.tsx` | GPU frame trace | unverified (no pinned GPU host) |
| Real authorized live feed traverses the pipeline | `aero-ingest/opensky.rs` | live run | blocked (no agreement) |
