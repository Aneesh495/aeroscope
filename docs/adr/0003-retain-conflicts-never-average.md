# ADR 0003: Retain conflicting evidence; never average positions or datums

## Status
Accepted.

## Context
When two sources report different positions for the same aircraft at close
event times, the tempting shortcut is to average them. Averaging erases genuine
disagreement, hides a bad source, and is meaningless across altitude datums
(barometric vs geometric) or timestamps whose meanings differ.

## Decision
The fusion engine keeps every contributing observation as `Evidence` on the
track and selects one source's position by a declared rule: higher declared
trust, then recency, otherwise keep the single source. The selected position is
always exactly one source's reported value, and the reason is recorded. Baro
and geo altitude are separate types with no cross conversion, so they cannot be
averaged even by accident.

## Consequences
- The displayed position is explainable: the drawer shows which source was
  chosen and why.
- Memory per track is bounded by pruning evidence older than a short window.
- A differential or adversarial test can assert the position equals a specific
  source's value, not a midpoint (see `higher_trust_source_wins_conflict`).
