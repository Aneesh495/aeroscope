# Latency accounting and source-clock error budget

Aeroscope separates two things that are easy to conflate: how old an
observation is, and how long Aeroscope took to process it. Only the second is
something the system can be held accountable for; the first is dominated by
upstream and clock effects.

## Definitions

Let an observation carry an observation time `t_obs` (if the source reports it),
a publish time `t_pub` (if available), and an ingest wall time `t_ing` recorded
when Aeroscope received the bytes. Fusion stamps `t_fuse` and delivery stamps
`t_del`.

- Observation age at ingest: `t_ing - t_obs`. This includes the sensor-to-source
  path, the source-to-Aeroscope network, and any clock offset between the source
  and Aeroscope. Aeroscope does not control it.
- Internal latency: `t_del - t_ing = (t_fuse - t_ing) + (t_del - t_fuse)`. This
  is the log + fusion time plus the subscription + encode time. This is the
  quantity gate G05 bounds (p95 < 150 ms, p99 < 500 ms at the declared load).

`LatencyBreakdown` in `aero-types::time` records these as separate fields and
`internal_ms()` returns only the internal part.

## Source-clock error budget

A source clock can disagree with Aeroscope's clock. If the offset is `delta`
(source ahead by `delta` ms), then the measured `t_ing - t_obs` is biased by
`-delta`: a source running fast makes observations look fresher than they are.

Aeroscope does not silently correct this. When `t_obs` is present the age is
reported as measured, and when a source's offset is known (for example from a
receiver that reports its own clock status) it is surfaced rather than folded
into the age. When `t_obs` is absent the age basis falls back to `t_pub` or
finally `t_ing`, and the basis is reported via `EventTimeBasis` so the UI can
say "age estimated from ingest time" instead of implying a precise observation
age.

## Why the watermark matters

The fusion watermark advances with event time, not wall time. A slow source
cannot stall timely sources because the watermark follows the fastest useful
evidence; a late observation older than `watermark - allowed_lateness` is
recorded as a late revision rather than dragging the live view backward. This
keeps internal latency bounded even when one source lags.

## Measurement

Internal latency samples are retained per observation in the capacity campaign;
p50/p95/p99 are computed over all samples, never a filtered subset. Observation
age is reported as a separate distribution. A pilot run on this host measures
only the bounded pilot; the full-host p95/p99 at the declared load is marked
unverified until executed on a declared reference host.
