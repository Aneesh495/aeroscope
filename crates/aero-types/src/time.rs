//! The Aeroscope clock model.
//!
//! The brief is explicit that internal processing latency is not observation
//! age, and that clock offsets and unavailable source timestamps must be
//! explicit. So we use distinct newtypes for each clock rather than passing
//! bare integers around. Mixing them up becomes a compile error, not a silent
//! off-by-a-whole-meaning bug.
//!
//! All wall-clock stamps are UTC milliseconds since the Unix epoch. The
//! monotonic ingest clock is nanoseconds from an arbitrary process-local zero
//! and must never be compared across processes or restarts.

use serde::{Deserialize, Serialize};

/// UTC milliseconds since the Unix epoch.
pub type UnixMillis = i64;

macro_rules! wall_clock {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(pub UnixMillis);

        impl $name {
            pub fn millis(self) -> UnixMillis { self.0 }
            /// Signed difference self - other in milliseconds.
            pub fn delta_ms(self, other: Self) -> i64 { self.0 - other.0 }
        }
    };
}

wall_clock!(
    /// When the aircraft state was actually observed at the source, if the
    /// source reports it. This is the time that matters for freshness.
    ObservationTime
);
wall_clock!(
    /// When the provider published the record, if available. May lag the
    /// observation time; the gap is real network/processing delay upstream.
    PublishTime
);
wall_clock!(
    /// Wall-clock time Aeroscope received the bytes. Subject to our own clock
    /// error relative to the source.
    IngestWallTime
);
wall_clock!(
    /// When fusion produced the state we are serving.
    FusedTime
);
wall_clock!(
    /// When a delta/snapshot left the server toward a subscriber.
    DeliveryTime
);

/// A monotonic, process-local clock in nanoseconds. Only valid for measuring
/// durations within one process run; never serialize it as an absolute time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonoNanos(pub u64);

impl MonoNanos {
    pub fn duration_ns_since(self, earlier: Self) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

/// A timestamp that a source may or may not provide. Keeping absence explicit
/// stops "no timestamp" from silently becoming "time zero" or "now".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaybeTime<T> {
    Known(T),
    /// The source did not provide this timestamp.
    Absent,
}

impl<T> MaybeTime<T> {
    pub fn known(self) -> Option<T> {
        match self {
            MaybeTime::Known(t) => Some(t),
            MaybeTime::Absent => None,
        }
    }
    pub fn is_known(&self) -> bool {
        matches!(self, MaybeTime::Known(_))
    }
}

/// Internal pipeline latency accounting for one observation, kept separate from
/// observation age. See `docs/LATENCY.md` for the derivation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LatencyBreakdown {
    /// ingest wall time minus observation time, if observation time is known.
    /// This is upstream + clock error, not something Aeroscope controls.
    pub observation_age_at_ingest_ms: Option<i64>,
    /// fused time minus ingest wall time: time spent in our log + fusion.
    pub ingest_to_fuse_ms: i64,
    /// delivery time minus fused time: time spent in subscription/encode.
    pub fuse_to_deliver_ms: i64,
}

impl LatencyBreakdown {
    /// Internal latency is the part Aeroscope is accountable for.
    pub fn internal_ms(&self) -> i64 {
        self.ingest_to_fuse_ms + self.fuse_to_deliver_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_are_distinct_types_but_compute_deltas() {
        let obs = ObservationTime(1_000);
        let ing = IngestWallTime(1_250);
        // We intentionally cannot subtract obs from ing directly; the breakdown
        // records the age explicitly.
        let lat = LatencyBreakdown {
            observation_age_at_ingest_ms: Some(ing.0 - obs.0),
            ingest_to_fuse_ms: 20,
            fuse_to_deliver_ms: 5,
        };
        assert_eq!(lat.observation_age_at_ingest_ms, Some(250));
        assert_eq!(lat.internal_ms(), 25);
    }

    #[test]
    fn absent_time_stays_absent() {
        let t: MaybeTime<ObservationTime> = MaybeTime::Absent;
        assert!(!t.is_known());
        assert_eq!(t.known(), None);
    }

    #[test]
    fn monotonic_duration_saturates() {
        let a = MonoNanos(100);
        let b = MonoNanos(40);
        assert_eq!(a.duration_ns_since(b), 60);
        // Out-of-order reads saturate to zero rather than wrapping.
        assert_eq!(b.duration_ns_since(a), 0);
    }
}
