//! Deterministic synthetic load generator (A33).
//!
//! Produces a reproducible stream of observations for globally distributed
//! aircraft, with seeded faults: source outages, duplicate records, and
//! out-of-order delivery. Everything is labeled synthetic; this exercises
//! known-truth correctness tests and the capacity campaign, and must never be
//! presented as a live feed or real coverage.
//!
//! The generator tracks a known ground truth (each aircraft's true great-circle
//! path), so a test can compare what the pipeline reconstructs against what was
//! actually emitted.

pub mod rng;

use aero_geo::{initial_bearing_deg, interpolate_great_circle, Geodetic};
use aero_types::{
    GeoAltM, GroundState, Icao24, IngestWallTime, MaybeTime, Observation, ObservationTime,
    SourceId, SpeedMps, TrueTrackDeg,
};
use rng::SplitMix64;

/// Ground truth for one synthetic aircraft: a great-circle leg at constant
/// speed between two random endpoints, looping.
#[derive(Debug, Clone)]
pub struct SyntheticAircraft {
    pub icao24: Icao24,
    pub origin: Geodetic,
    pub dest: Geodetic,
    pub cruise_alt_m: f64,
    pub speed_mps: f64,
    /// Seconds to traverse the whole leg.
    pub leg_duration_s: f64,
}

impl SyntheticAircraft {
    /// True position at time `t_s` seconds into the (looping) leg.
    pub fn true_position(&self, t_s: f64) -> Geodetic {
        let frac = (t_s / self.leg_duration_s).rem_euclid(1.0);
        interpolate_great_circle(&self.origin, &self.dest, frac)
    }

    pub fn true_track_deg(&self, t_s: f64) -> f64 {
        let frac = (t_s / self.leg_duration_s).rem_euclid(1.0);
        let a = self.true_position(t_s);
        let b = self.true_position(t_s + self.leg_duration_s * (frac + 0.01).min(1.0) * 0.0 + 1.0);
        initial_bearing_deg(&a, &b)
    }
}

pub struct LoadGenConfig {
    pub aircraft_count: usize,
    pub seed: u64,
    /// Probability per emitted record that it is duplicated.
    pub duplicate_prob: f64,
    /// Probability per record that it is delayed (emitted out of order).
    pub reorder_prob: f64,
    /// Probability per tick that a given source is in an outage.
    pub outage_prob: f64,
    pub source_id: String,
}

impl Default for LoadGenConfig {
    fn default() -> Self {
        Self {
            aircraft_count: 1000,
            seed: 1,
            duplicate_prob: 0.01,
            reorder_prob: 0.02,
            outage_prob: 0.0,
            source_id: "synthetic".into(),
        }
    }
}

pub struct LoadGenerator {
    pub fleet: Vec<SyntheticAircraft>,
    cfg: LoadGenConfig,
    rng: SplitMix64,
    /// Counters for independent verification (G03/G04).
    pub emitted: u64,
    pub duplicates: u64,
    pub reordered: u64,
    pub outage_skipped: u64,
}

impl LoadGenerator {
    pub fn new(cfg: LoadGenConfig) -> Self {
        let mut rng = SplitMix64::new(cfg.seed);
        let mut fleet = Vec::with_capacity(cfg.aircraft_count);
        for i in 0..cfg.aircraft_count {
            // Globally distributed endpoints.
            let origin = Geodetic::new(
                rng.range(-80.0, 80.0),
                rng.range(-180.0, 180.0),
                0.0,
            );
            let dest = Geodetic::new(
                rng.range(-80.0, 80.0),
                rng.range(-180.0, 180.0),
                0.0,
            );
            // ICAO24 derived deterministically from the index.
            let bytes = [(i >> 16) as u8, (i >> 8) as u8, (i as u8).max(1)];
            let speed_mps = rng.range(200.0, 260.0);
            // Leg duration is derived from the real great-circle distance and
            // the cruise speed so the aircraft moves at a physically plausible
            // rate; random endpoints with a fixed duration would otherwise imply
            // impossible speeds and the fusion plausibility check would (rightly)
            // reject almost everything.
            let dist_m = aero_geo::haversine_m(&origin, &dest).max(1.0);
            let leg_duration_s = (dist_m / speed_mps).max(600.0);
            fleet.push(SyntheticAircraft {
                icao24: Icao24(bytes),
                origin,
                dest,
                cruise_alt_m: rng.range(8000.0, 12000.0),
                speed_mps,
                leg_duration_s,
            });
        }
        Self {
            fleet,
            cfg,
            rng,
            emitted: 0,
            duplicates: 0,
            reordered: 0,
            outage_skipped: 0,
        }
    }

    /// Emit one tick of observations at wall time `now_ms`, with `t_s` being the
    /// elapsed seconds used to evaluate the true positions. Returns observations
    /// including any injected duplicates; out-of-order records carry an older
    /// observation time so the fusion engine must handle lateness.
    pub fn tick(&mut self, now_ms: i64, t_s: f64) -> Vec<Observation> {
        // Decide outage for this source this tick.
        if self.cfg.outage_prob > 0.0 && self.rng.chance(self.cfg.outage_prob) {
            self.outage_skipped += self.fleet.len() as u64;
            return Vec::new();
        }
        let src = SourceId(self.cfg.source_id.clone());
        let mut out = Vec::with_capacity(self.fleet.len());
        // Snapshot the probabilities to avoid borrowing self in the loop.
        let dup_p = self.cfg.duplicate_prob;
        let reorder_p = self.cfg.reorder_prob;
        let fleet = self.fleet.clone();
        for ac in &fleet {
            let pos = ac.true_position(t_s);
            let positioned = Geodetic::new(pos.lat_deg, pos.lon_deg, ac.cruise_alt_m);
            let reorder = self.rng.chance(reorder_p);
            // A reordered record is stamped a few seconds in the past.
            let obs_time = if reorder {
                self.reordered += 1;
                now_ms - 8_000
            } else {
                now_ms
            };
            let o = Observation {
                source: src.clone(),
                icao24: ac.icao24,
                callsign: None,
                observation_time: MaybeTime::Known(ObservationTime(obs_time)),
                publish_time: MaybeTime::Absent,
                ingest_wall_time: IngestWallTime(now_ms),
                position: Some(positioned),
                baro_alt: None,
                geo_alt: Some(GeoAltM(ac.cruise_alt_m)),
                ground_speed: Some(SpeedMps(ac.speed_mps)),
                true_track: Some(TrueTrackDeg(ac.true_track_deg(t_s))),
                vertical_rate: None,
                ground_state: GroundState::Airborne,
                source_sequence: Some(self.emitted),
            };
            self.emitted += 1;
            if self.rng.chance(dup_p) {
                self.duplicates += 1;
                out.push(o.clone());
            }
            out.push(o);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fleet_is_global_and_reproducible() {
        let a = LoadGenerator::new(LoadGenConfig { aircraft_count: 500, seed: 9, ..Default::default() });
        let b = LoadGenerator::new(LoadGenConfig { aircraft_count: 500, seed: 9, ..Default::default() });
        assert_eq!(a.fleet.len(), 500);
        // Same seed -> identical fleet endpoints.
        for (x, y) in a.fleet.iter().zip(b.fleet.iter()) {
            assert_eq!(x.origin.lat_deg, y.origin.lat_deg);
            assert_eq!(x.origin.lon_deg, y.origin.lon_deg);
        }
        // Spread across hemispheres.
        let any_south = a.fleet.iter().any(|f| f.origin.lat_deg < 0.0);
        let any_north = a.fleet.iter().any(|f| f.origin.lat_deg > 0.0);
        assert!(any_south && any_north);
    }

    #[test]
    fn tick_emits_one_record_per_aircraft_plus_duplicates() {
        let mut g = LoadGenerator::new(LoadGenConfig {
            aircraft_count: 100,
            seed: 3,
            duplicate_prob: 0.0,
            reorder_prob: 0.0,
            ..Default::default()
        });
        let out = g.tick(1_000_000, 10.0);
        assert_eq!(out.len(), 100);
        assert_eq!(g.emitted, 100);
    }

    #[test]
    fn duplicates_and_reorders_are_counted() {
        let mut g = LoadGenerator::new(LoadGenConfig {
            aircraft_count: 2000,
            seed: 5,
            duplicate_prob: 0.1,
            reorder_prob: 0.1,
            ..Default::default()
        });
        let out = g.tick(1_000_000, 10.0);
        assert!(g.duplicates > 0, "expected some duplicates");
        assert!(g.reordered > 0, "expected some reorders");
        // Output length is fleet + duplicates.
        assert_eq!(out.len(), 2000 + g.duplicates as usize);
    }

    #[test]
    fn true_position_moves_over_time() {
        let g = LoadGenerator::new(LoadGenConfig { aircraft_count: 1, seed: 11, ..Default::default() });
        let ac = &g.fleet[0];
        let p0 = ac.true_position(0.0);
        let p1 = ac.true_position(ac.leg_duration_s * 0.25);
        let moved = aero_geo::haversine_m(&p0, &p1);
        assert!(moved > 1000.0, "aircraft should move, moved {moved} m");
    }
}
