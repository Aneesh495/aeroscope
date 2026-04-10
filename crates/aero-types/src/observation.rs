//! The normalized observation: the atom that flows through the whole pipeline.
//!
//! An observation is one source's report of one aircraft at one instant, after
//! parsing and unit normalization but before fusion. Every optional field is a
//! real `Option` so "the source did not send this" is distinct from a default.

use crate::identity::{Callsign, Icao24};
use crate::time::{IngestWallTime, MaybeTime, ObservationTime, PublishTime};
use crate::units::{BaroAltM, GeoAltM, SpeedMps, TrueTrackDeg, VerticalRateMps};
use aero_geo::Geodetic;
use serde::{Deserialize, Serialize};

/// Which source produced an observation, for attribution and reliability.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub String);

/// Whether the aircraft is on the ground, per the source. Grounded taxi motion
/// is real and must not be mistaken for airborne flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundState {
    Airborne,
    OnGround,
    Unknown,
}

/// A normalized observation. Position is optional because many Mode S messages
/// carry identity or velocity without a position; a position-less observation
/// is still useful and must never be turned into a fabricated location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub source: SourceId,
    pub icao24: Icao24,
    pub callsign: Option<Callsign>,

    pub observation_time: MaybeTime<ObservationTime>,
    pub publish_time: MaybeTime<PublishTime>,
    pub ingest_wall_time: IngestWallTime,

    /// WGS84 position with geometric altitude, if the source gave a position.
    pub position: Option<Geodetic>,
    /// Barometric altitude preserved separately from geometric.
    pub baro_alt: Option<BaroAltM>,
    pub geo_alt: Option<GeoAltM>,

    pub ground_speed: Option<SpeedMps>,
    pub true_track: Option<TrueTrackDeg>,
    pub vertical_rate: Option<VerticalRateMps>,
    pub ground_state: GroundState,

    /// Monotonically increasing sequence within one source stream, used to
    /// detect gaps. None if the source does not provide one.
    pub source_sequence: Option<u64>,
}

impl Observation {
    /// The time we trust for freshness: observation time if known, else publish
    /// time, else ingest wall time. The chosen basis is reported, never hidden.
    pub fn effective_event_time_ms(&self) -> (i64, EventTimeBasis) {
        if let Some(o) = self.observation_time.known() {
            (o.0, EventTimeBasis::Observation)
        } else if let Some(p) = self.publish_time.known() {
            (p.0, EventTimeBasis::Publish)
        } else {
            (self.ingest_wall_time.0, EventTimeBasis::IngestFallback)
        }
    }

    /// An observation with no position still validates; it just cannot place an
    /// aircraft. Returns false only for physically impossible stored values.
    pub fn is_structurally_valid(&self) -> bool {
        if self.icao24.is_reserved() {
            return false;
        }
        if let Some(p) = &self.position {
            if !p.is_valid() {
                return false;
            }
        }
        true
    }
}

/// Which clock the effective event time came from, so downstream code and the
/// UI can show the assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventTimeBasis {
    Observation,
    Publish,
    IngestFallback,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Observation {
        Observation {
            source: SourceId("test".into()),
            icao24: Icao24::parse_hex("4ca7b3").unwrap(),
            callsign: Callsign::normalize("DLH42"),
            observation_time: MaybeTime::Absent,
            publish_time: MaybeTime::Absent,
            ingest_wall_time: IngestWallTime(5_000),
            position: Some(Geodetic::new(51.47, -0.45, 10_000.0)),
            baro_alt: Some(BaroAltM(10_000.0)),
            geo_alt: Some(GeoAltM(10_050.0)),
            ground_speed: Some(SpeedMps(230.0)),
            true_track: Some(TrueTrackDeg(270.0)),
            vertical_rate: Some(VerticalRateMps(0.0)),
            ground_state: GroundState::Airborne,
            source_sequence: Some(1),
        }
    }

    #[test]
    fn event_time_prefers_observation_then_publish_then_ingest() {
        let mut o = base();
        let (_, basis) = o.effective_event_time_ms();
        assert_eq!(basis, EventTimeBasis::IngestFallback);

        o.publish_time = MaybeTime::Known(PublishTime(4_000));
        let (t, basis) = o.effective_event_time_ms();
        assert_eq!(t, 4_000);
        assert_eq!(basis, EventTimeBasis::Publish);

        o.observation_time = MaybeTime::Known(ObservationTime(3_000));
        let (t, basis) = o.effective_event_time_ms();
        assert_eq!(t, 3_000);
        assert_eq!(basis, EventTimeBasis::Observation);
    }

    #[test]
    fn positionless_observation_is_still_valid() {
        let mut o = base();
        o.position = None;
        assert!(o.is_structurally_valid());
    }

    #[test]
    fn reserved_icao_is_invalid() {
        let mut o = base();
        o.icao24 = Icao24::parse_hex("000000").unwrap();
        assert!(!o.is_structurally_valid());
    }
}
