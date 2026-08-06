//! Plausibility checks (A07).
//!
//! Before an observation updates a track we test it against the previous state:
//! impossible jumps, time reversal, invalid coordinates, and ground/air
//! contradictions. A failing check does not delete the observation (it stays in
//! the log); it is flagged so fusion can retain the conflict rather than letting
//! a bad decode teleport an aircraft.

use aero_geo::{haversine_m, Geodetic};
use aero_types::Observation;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Plausibility {
    Ok,
    /// Position implies a speed above any real aircraft.
    ImpossibleJump,
    /// Event time goes backwards relative to the track's latest.
    TimeReversal,
    /// Coordinates are outside the valid geodetic range or non-finite.
    InvalidCoordinates,
    /// Source says on-ground but the implied speed/altitude says airborne.
    GroundAirConflict,
}

/// Maximum plausible ground speed for fixed-wing traffic, m/s (~Mach 3 margin).
const MAX_SPEED_MPS: f64 = 1_000.0;

/// Check a new observation against the previous accepted position/time.
pub fn check(
    prev_pos: Option<Geodetic>,
    prev_event_ms: Option<i64>,
    obs: &Observation,
) -> Plausibility {
    if let Some(p) = &obs.position {
        if !p.is_valid() {
            return Plausibility::InvalidCoordinates;
        }
    }
    let (new_ms, _) = obs.effective_event_time_ms();
    if let Some(prev_ms) = prev_event_ms {
        if new_ms < prev_ms {
            return Plausibility::TimeReversal;
        }
        if let (Some(prev), Some(cur)) = (prev_pos, obs.position) {
            let dt_s = (new_ms - prev_ms) as f64 / 1000.0;
            if dt_s > 0.0 {
                let dist = haversine_m(&prev, &cur);
                let speed = dist / dt_s;
                if speed > MAX_SPEED_MPS {
                    return Plausibility::ImpossibleJump;
                }
            }
        }
    }
    // Ground/air conflict: source says on-ground but reporting a large baro
    // altitude and high speed. We only flag the clear contradiction.
    if matches!(obs.ground_state, aero_types::GroundState::OnGround) {
        if let (Some(baro), Some(gs)) = (obs.baro_alt, obs.ground_speed) {
            if baro.0 > 500.0 && gs.0 > 60.0 {
                return Plausibility::GroundAirConflict;
            }
        }
    }
    Plausibility::Ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_types::*;

    fn obs_at(lat: f64, lon: f64, t: i64) -> Observation {
        Observation {
            source: SourceId("s".into()),
            icao24: Icao24::parse_hex("4ca7b3").unwrap(),
            callsign: None,
            observation_time: MaybeTime::Known(ObservationTime(t)),
            publish_time: MaybeTime::Absent,
            ingest_wall_time: IngestWallTime(t),
            position: Some(Geodetic::new(lat, lon, 10000.0)),
            baro_alt: Some(BaroAltM(10000.0)),
            geo_alt: Some(GeoAltM(10000.0)),
            ground_speed: Some(SpeedMps(230.0)),
            true_track: Some(TrueTrackDeg(90.0)),
            vertical_rate: Some(VerticalRateMps(0.0)),
            ground_state: GroundState::Airborne,
            source_sequence: None,
        }
    }

    #[test]
    fn normal_movement_is_ok() {
        let prev = Geodetic::new(51.0, 0.0, 10000.0);
        // ~230 m/s over 10 s -> ~2.3 km, fine.
        let o = obs_at(51.0, 0.03, 11_000);
        assert_eq!(check(Some(prev), Some(1_000), &o), Plausibility::Ok);
    }

    #[test]
    fn teleport_flagged_as_impossible_jump() {
        let prev = Geodetic::new(51.0, 0.0, 10000.0);
        // Half the planet in one second.
        let o = obs_at(-30.0, 150.0, 2_000);
        assert_eq!(
            check(Some(prev), Some(1_000), &o),
            Plausibility::ImpossibleJump
        );
    }

    #[test]
    fn backwards_time_flagged() {
        let o = obs_at(51.0, 0.0, 500);
        assert_eq!(check(None, Some(1_000), &o), Plausibility::TimeReversal);
    }

    #[test]
    fn ground_air_conflict_flagged() {
        let mut o = obs_at(51.0, 0.0, 2_000);
        o.ground_state = GroundState::OnGround; // but baro 10000 m and 230 m/s
        assert_eq!(check(None, None, &o), Plausibility::GroundAirConflict);
    }
}
