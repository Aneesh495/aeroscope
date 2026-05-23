//! Track lifecycle states (A06).
//!
//! A track moves through a small, declared set of states based on how recently
//! it was observed. Extrapolation is deliberately short-lived: past a declared
//! horizon we stop pretending we know where the aircraft is and mark it stale,
//! then lost. The horizons are configuration, not magic numbers buried in code.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackState {
    /// A fresh observation arrived within the observed window.
    Observed,
    /// No fresh observation, but within the interpolation window between two
    /// real observations (used only for replay/history, not live extrapolation).
    Interpolated,
    /// Briefly past the last observation; position is extrapolated from the last
    /// known velocity for at most `extrapolation_horizon_ms`.
    Extrapolated,
    /// Past the extrapolation horizon; we no longer move the symbol and flag it.
    Stale,
    /// Past the stale horizon; the track is considered lost.
    Lost,
    /// Explicitly ended (for example landed and stopped reporting on ground).
    Ended,
}

/// Declared horizons for lifecycle transitions, all in milliseconds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LifecycleConfig {
    /// Within this age an observation is still "fresh".
    pub observed_window_ms: i64,
    /// How long we allow short-term extrapolation after the last observation.
    pub extrapolation_horizon_ms: i64,
    /// After this age the track is lost.
    pub stale_horizon_ms: i64,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        // Tuned for ~1 Hz feeds: fresh for 5 s, extrapolate up to 15 s, lost at
        // 60 s. These are the declared horizons referenced by A06 and G07.
        Self {
            observed_window_ms: 5_000,
            extrapolation_horizon_ms: 15_000,
            stale_horizon_ms: 60_000,
        }
    }
}

impl LifecycleConfig {
    /// Classify a track given the age of its most recent observation.
    pub fn classify(&self, age_ms: i64) -> TrackState {
        if age_ms < 0 {
            // A future-dated observation is a clock problem, not freshness.
            return TrackState::Observed;
        }
        if age_ms <= self.observed_window_ms {
            TrackState::Observed
        } else if age_ms <= self.observed_window_ms + self.extrapolation_horizon_ms {
            TrackState::Extrapolated
        } else if age_ms <= self.stale_horizon_ms {
            TrackState::Stale
        } else {
            TrackState::Lost
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitions_follow_declared_horizons() {
        let c = LifecycleConfig::default();
        assert_eq!(c.classify(0), TrackState::Observed);
        assert_eq!(c.classify(5_000), TrackState::Observed);
        assert_eq!(c.classify(5_001), TrackState::Extrapolated);
        assert_eq!(c.classify(20_000), TrackState::Extrapolated);
        assert_eq!(c.classify(20_001), TrackState::Stale);
        assert_eq!(c.classify(60_000), TrackState::Stale);
        assert_eq!(c.classify(60_001), TrackState::Lost);
    }

    #[test]
    fn future_observation_is_observed_not_stale() {
        let c = LifecycleConfig::default();
        assert_eq!(c.classify(-1000), TrackState::Observed);
    }
}
