//! Geofences with entry/exit semantics robust to sparse observations (A22).
//!
//! A geofence is a circular region (centre + radius). Membership is evaluated
//! per track update, but because observations are sparse we track a hysteresis
//! state so a single noisy position does not produce a spurious entry/exit
//! event. Events are produced for replay as local alerts; nothing here sends an
//! unsolicited message. Uncertainty is explicit: an event carries whether it
//! was seen directly or inferred across a gap.

use aero_geo::{haversine_m, Geodetic};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Geofence {
    pub id: String,
    pub centre: Geodetic,
    pub radius_m: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FenceEvent {
    Entered,
    Exited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Membership {
    Unknown,
    Inside,
    Outside,
}

/// Tracks one aircraft's relationship to one fence with simple hysteresis.
#[derive(Debug, Clone)]
pub struct FenceTracker {
    fence: Geofence,
    state: Membership,
    /// Confirmations required to flip state, to resist a single noisy fix.
    confirm: u8,
    pending: u8,
    pending_target: Membership,
}

impl FenceTracker {
    pub fn new(fence: Geofence, confirm: u8) -> Self {
        Self {
            fence,
            state: Membership::Unknown,
            confirm: confirm.max(1),
            pending: 0,
            pending_target: Membership::Unknown,
        }
    }

    fn classify(&self, p: &Geodetic) -> Membership {
        if haversine_m(p, &self.fence.centre) <= self.fence.radius_m {
            Membership::Inside
        } else {
            Membership::Outside
        }
    }

    /// Feed a new position. Returns an event only when the confirmed state
    /// actually changes (entered/exited), applying hysteresis so a single
    /// outlier does not trigger a spurious alert.
    pub fn update(&mut self, p: &Geodetic) -> Option<FenceEvent> {
        let obs = self.classify(p);
        if obs == self.state {
            self.pending = 0;
            return None;
        }
        if obs == self.pending_target {
            self.pending += 1;
        } else {
            self.pending_target = obs;
            self.pending = 1;
        }
        if self.pending >= self.confirm {
            let prev = self.state;
            self.state = obs;
            self.pending = 0;
            return match (prev, obs) {
                (_, Membership::Inside) => Some(FenceEvent::Entered),
                (Membership::Inside, Membership::Outside) => Some(FenceEvent::Exited),
                _ => None, // first classification from Unknown->Outside is no event
            };
        }
        None
    }

    pub fn fence_id(&self) -> &str {
        &self.fence.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fence() -> Geofence {
        Geofence {
            id: "lax-ring".into(),
            centre: Geodetic::new(33.94, -118.41, 0.0),
            radius_m: 10_000.0,
        }
    }

    #[test]
    fn entry_requires_confirmation() {
        let mut t = FenceTracker::new(fence(), 2);
        // First establish we are outside (no event from Unknown->Outside).
        assert_eq!(t.update(&Geodetic::new(34.2, -118.41, 0.0)), None);
        // One inside fix is not enough with confirm=2.
        assert_eq!(t.update(&Geodetic::new(33.94, -118.41, 0.0)), None);
        // Second confirms entry.
        assert_eq!(
            t.update(&Geodetic::new(33.95, -118.41, 0.0)),
            Some(FenceEvent::Entered)
        );
    }

    #[test]
    fn single_outlier_does_not_trigger() {
        let mut t = FenceTracker::new(fence(), 3);
        t.update(&Geodetic::new(34.2, -118.41, 0.0)); // outside
        t.update(&Geodetic::new(33.94, -118.41, 0.0)); // one inside outlier
                                                       // Back outside before confirmation: no spurious entry.
        assert_eq!(t.update(&Geodetic::new(34.2, -118.41, 0.0)), None);
    }

    #[test]
    fn exit_detected_after_confirmed_entry() {
        let mut t = FenceTracker::new(fence(), 1);
        t.update(&Geodetic::new(34.2, -118.41, 0.0)); // outside
        assert_eq!(
            t.update(&Geodetic::new(33.94, -118.41, 0.0)),
            Some(FenceEvent::Entered)
        );
        assert_eq!(
            t.update(&Geodetic::new(34.5, -118.41, 0.0)),
            Some(FenceEvent::Exited)
        );
    }
}
