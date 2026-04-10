//! Core domain types for Aeroscope: clocks, units, identities, the normalized
//! observation, and the ingestion error contract. These types encode the
//! honesty invariants from `CONTRACT.md` directly in the type system so that
//! misusing a clock or an altitude datum is a compile error.

pub mod error;
pub mod identity;
pub mod observation;
pub mod time;
pub mod units;

pub use error::{IngestError, QuarantinedRecord};
pub use identity::{Callsign, Enriched, Icao24, Provenance};
pub use observation::{EventTimeBasis, GroundState, Observation, SourceId};
pub use time::{
    DeliveryTime, FusedTime, IngestWallTime, LatencyBreakdown, MaybeTime, MonoNanos,
    ObservationTime, PublishTime, UnixMillis,
};
pub use units::{BaroAltM, GeoAltM, HeadingDeg, SpeedMps, TrueTrackDeg, VerticalRateMps};
