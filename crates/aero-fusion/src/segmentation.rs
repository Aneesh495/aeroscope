//! Flight-leg segmentation (A08).
//!
//! A single airframe flies many legs in a day, and a callsign is neither unique
//! nor stable, so we cannot split legs by callsign alone. We segment a track's
//! observations into legs using evidence: a long on-ground dwell, a long
//! observation gap, or a callsign change each end a leg. Every boundary records
//! why it was drawn and how confident we are, so a weak signal is not presented
//! as a certain new flight.

use aero_types::{GroundState, Observation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegBoundaryCause {
    GroundDwell,
    ObservationGap,
    CallsignChange,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlightLeg {
    pub start_event_ms: i64,
    pub end_event_ms: i64,
    pub observation_count: usize,
    pub start_callsign: Option<String>,
    /// Why this leg ended (None for the final, still-open leg).
    pub end_cause: Option<LegBoundaryCause>,
    /// 0..1 confidence that this is genuinely a distinct leg.
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct SegmentationConfig {
    /// A gap longer than this ends a leg.
    pub max_gap_ms: i64,
    /// Continuous on-ground time longer than this ends a leg.
    pub ground_dwell_ms: i64,
}

impl Default for SegmentationConfig {
    fn default() -> Self {
        Self { max_gap_ms: 600_000, ground_dwell_ms: 300_000 }
    }
}

/// Segment chronologically ordered observations into legs. Input need not be
/// sorted; we sort by effective event time first (A08 is order-independent).
pub fn segment(obs: &[Observation], cfg: SegmentationConfig) -> Vec<FlightLeg> {
    if obs.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<&Observation> = obs.iter().collect();
    sorted.sort_by_key(|o| o.effective_event_time_ms().0);

    let mut legs = Vec::new();
    let mut leg_start = sorted[0].effective_event_time_ms().0;
    let mut leg_start_cs = sorted[0].callsign.as_ref().map(|c| c.0.clone());
    let mut count = 1usize;
    let mut prev_ms = leg_start;
    let mut ground_since: Option<i64> = None;
    let mut last_cs = leg_start_cs.clone();

    for o in sorted.iter().skip(1) {
        let (t, _) = o.effective_event_time_ms();
        let cs = o.callsign.as_ref().map(|c| c.0.clone());

        // Track continuous ground time.
        match o.ground_state {
            GroundState::OnGround => {
                ground_since.get_or_insert(prev_ms);
            }
            _ => ground_since = None,
        }

        let gap = t - prev_ms;
        let ground_dwell = ground_since.map(|s| t - s).unwrap_or(0);

        let cause = if gap > cfg.max_gap_ms {
            Some((LegBoundaryCause::ObservationGap, 0.9))
        } else if ground_dwell > cfg.ground_dwell_ms {
            Some((LegBoundaryCause::GroundDwell, 0.8))
        } else if cs.is_some() && last_cs.is_some() && cs != last_cs {
            Some((LegBoundaryCause::CallsignChange, 0.6))
        } else {
            None
        };

        if let Some((boundary, conf)) = cause {
            legs.push(FlightLeg {
                start_event_ms: leg_start,
                end_event_ms: prev_ms,
                observation_count: count,
                start_callsign: leg_start_cs.clone(),
                end_cause: Some(boundary),
                confidence: conf,
            });
            leg_start = t;
            leg_start_cs = cs.clone();
            count = 1;
            ground_since = None;
        } else {
            count += 1;
        }

        prev_ms = t;
        if cs.is_some() {
            last_cs = cs;
        }
    }

    // Final open leg.
    legs.push(FlightLeg {
        start_event_ms: leg_start,
        end_event_ms: prev_ms,
        observation_count: count,
        start_callsign: leg_start_cs,
        end_cause: None,
        confidence: 1.0,
    });
    legs
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geo::Geodetic;
    use aero_types::*;

    fn o(cs: Option<&str>, t: i64, ground: GroundState) -> Observation {
        Observation {
            source: SourceId("s".into()),
            icao24: Icao24::parse_hex("4ca7b3").unwrap(),
            callsign: cs.and_then(Callsign::normalize),
            observation_time: MaybeTime::Known(ObservationTime(t)),
            publish_time: MaybeTime::Absent,
            ingest_wall_time: IngestWallTime(t),
            position: Some(Geodetic::new(51.0, 0.0, 10000.0)),
            baro_alt: None,
            geo_alt: Some(GeoAltM(10000.0)),
            ground_speed: Some(SpeedMps(230.0)),
            true_track: None,
            vertical_rate: None,
            ground_state: ground,
            source_sequence: None,
        }
    }

    #[test]
    fn single_continuous_track_is_one_leg() {
        let obs = vec![
            o(Some("DLH1"), 0, GroundState::Airborne),
            o(Some("DLH1"), 60_000, GroundState::Airborne),
            o(Some("DLH1"), 120_000, GroundState::Airborne),
        ];
        let legs = segment(&obs, SegmentationConfig::default());
        assert_eq!(legs.len(), 1);
        assert_eq!(legs[0].observation_count, 3);
        assert!(legs[0].end_cause.is_none());
    }

    #[test]
    fn long_gap_splits_into_two_legs() {
        let obs = vec![
            o(Some("DLH1"), 0, GroundState::Airborne),
            o(Some("DLH1"), 60_000, GroundState::Airborne),
            // 20 min gap > 10 min threshold.
            o(Some("DLH1"), 60_000 + 1_200_000, GroundState::Airborne),
        ];
        let legs = segment(&obs, SegmentationConfig::default());
        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].end_cause, Some(LegBoundaryCause::ObservationGap));
    }

    #[test]
    fn callsign_change_splits_with_lower_confidence() {
        let obs = vec![
            o(Some("DLH1"), 0, GroundState::Airborne),
            o(Some("DLH2"), 60_000, GroundState::Airborne),
        ];
        let legs = segment(&obs, SegmentationConfig::default());
        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].end_cause, Some(LegBoundaryCause::CallsignChange));
        assert!(legs[0].confidence < 0.8);
    }

    #[test]
    fn unsorted_input_is_sorted_first() {
        let obs = vec![
            o(Some("DLH1"), 120_000, GroundState::Airborne),
            o(Some("DLH1"), 0, GroundState::Airborne),
            o(Some("DLH1"), 60_000, GroundState::Airborne),
        ];
        let legs = segment(&obs, SegmentationConfig::default());
        assert_eq!(legs.len(), 1);
        assert_eq!(legs[0].start_event_ms, 0);
        assert_eq!(legs[0].end_event_ms, 120_000);
    }
}
