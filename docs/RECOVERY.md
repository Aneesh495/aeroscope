# Recovery runbook

This runbook covers the failure modes Aeroscope is designed to survive and how
to confirm recovery (gate G08).

## Crash during log append

Symptom: the process was killed while writing an observation frame.

Behavior: on restart, `ObservationLog::open` scans frames from the start. A
frame whose magic, length, or CRC fails to validate marks the end of the intact
region, and the file is truncated there. All earlier records survive and the
dedup index is rebuilt from them.

Confirm: reopen the log and compare `count()` against the number of
independently known accepted records. The torn-tail test in `aero-log`
exercises exactly this path.

## Source outage

Symptom: a configured source stops responding or returns errors.

Behavior: the quota scheduler (planned in `aero-ingest` capability layer) opens
a circuit breaker after repeated failures, retries with bounded exponential
backoff and jitter, and surfaces the outage status to the UI. Ingestion of
other sources is unaffected because each source is isolated.

Confirm: the UI source badge shows the outage; request logs stay within quota;
retries are bounded.

## Client reconnect

Symptom: a subscriber's connection drops and reconnects.

Behavior: the client sends `Resume(last_snapshot_id, last_seq)`. If its snapshot
id still matches the subscription's current snapshot, the server continues with
deltas; otherwise it sends a fresh snapshot. A delta is never applied on top of
a mismatched or gapped snapshot (the resync invariant), so stale state cannot
accumulate.

Confirm: the client `TrackStore` reports `resyncNeeded` on a mismatch and clears
it after the next snapshot. The protocol tests cover both the match and
mismatch paths.

## Slow consumer

Symptom: a subscriber cannot keep up with updates.

Behavior: the subscription coalesces pending per-track updates to the latest
value. If the coalesced backlog still exceeds the hard cap, the server drops the
backlog and forces a resync rather than blocking ingestion or growing memory.

Confirm: the subscription emits `ResyncRequired` under sustained overload; the
`overflowing_hard_cap_forces_resync` test demonstrates it.
