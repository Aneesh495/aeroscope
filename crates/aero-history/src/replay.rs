//! Reproducible replay controller (A13).
//!
//! Replay advances an event-time cursor over stored observations at a variable
//! speed, with pause, scrub, and loop. The controller is pure logic over a time
//! cursor; it is deterministic and never presents itself as live. The
//! `is_live()` method returns false unconditionally so no UI path can mistake a
//! replay session for a live feed (honesty invariant, G07).

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayState {
    Playing,
    Paused,
}

/// Controls an event-time cursor between `start_ms` and `end_ms`.
#[derive(Debug, Clone)]
pub struct ReplayController {
    start_ms: i64,
    end_ms: i64,
    cursor_ms: i64,
    speed: f64,
    looping: bool,
    state: PlayState,
}

impl ReplayController {
    pub fn new(start_ms: i64, end_ms: i64) -> Self {
        assert!(end_ms >= start_ms, "replay window must be non-empty");
        Self {
            start_ms,
            end_ms,
            cursor_ms: start_ms,
            speed: 1.0,
            looping: false,
            state: PlayState::Paused,
        }
    }

    /// Replay is never live. This is deliberately unconditional.
    pub fn is_live(&self) -> bool {
        false
    }

    pub fn play(&mut self) {
        self.state = PlayState::Playing;
    }
    pub fn pause(&mut self) {
        self.state = PlayState::Paused;
    }
    pub fn state(&self) -> PlayState {
        self.state
    }
    pub fn cursor_ms(&self) -> i64 {
        self.cursor_ms
    }
    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// Set speed. Clamped to a sane positive range; zero or negative speeds are
    /// expressed by pausing, not by a degenerate multiplier.
    pub fn set_speed(&mut self, speed: f64) {
        self.speed = speed.clamp(0.1, 64.0);
    }

    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    /// Scrub directly to an event time, clamped to the window.
    pub fn scrub_to(&mut self, event_ms: i64) {
        self.cursor_ms = event_ms.clamp(self.start_ms, self.end_ms);
    }

    /// Advance the cursor by `wall_dt_ms` of real time, scaled by speed. At the
    /// end it either loops to the start or pauses. Returns the (from, to) event
    /// window covered, so a caller can emit exactly the observations inside it.
    pub fn advance(&mut self, wall_dt_ms: i64) -> (i64, i64) {
        if self.state == PlayState::Paused {
            return (self.cursor_ms, self.cursor_ms);
        }
        let from = self.cursor_ms;
        let delta = (wall_dt_ms as f64 * self.speed).round() as i64;
        let mut to = self.cursor_ms + delta;
        if to >= self.end_ms {
            if self.looping {
                to = self.end_ms;
                self.cursor_ms = self.start_ms;
            } else {
                to = self.end_ms;
                self.cursor_ms = self.end_ms;
                self.state = PlayState::Paused;
            }
        } else {
            self.cursor_ms = to;
        }
        (from, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_is_never_live() {
        let r = ReplayController::new(0, 1000);
        assert!(!r.is_live());
    }

    #[test]
    fn advances_at_speed() {
        let mut r = ReplayController::new(0, 100_000);
        r.set_speed(4.0);
        r.play();
        let (from, to) = r.advance(1000); // 1 s wall at 4x -> 4 s event
        assert_eq!(from, 0);
        assert_eq!(to, 4000);
        assert_eq!(r.cursor_ms(), 4000);
    }

    #[test]
    fn pause_freezes_cursor() {
        let mut r = ReplayController::new(0, 100_000);
        r.play();
        r.advance(1000);
        r.pause();
        let c = r.cursor_ms();
        let (from, to) = r.advance(1000);
        assert_eq!(from, c);
        assert_eq!(to, c);
    }

    #[test]
    fn reaches_end_and_pauses_without_loop() {
        let mut r = ReplayController::new(0, 5000);
        r.play();
        r.advance(10_000); // overshoots
        assert_eq!(r.cursor_ms(), 5000);
        assert_eq!(r.state(), PlayState::Paused);
    }

    #[test]
    fn loops_back_to_start() {
        let mut r = ReplayController::new(0, 5000);
        r.set_looping(true);
        r.play();
        r.advance(10_000);
        assert_eq!(r.cursor_ms(), 0);
        assert_eq!(r.state(), PlayState::Playing);
    }

    #[test]
    fn scrub_clamps_to_window() {
        let mut r = ReplayController::new(1000, 5000);
        r.scrub_to(9999);
        assert_eq!(r.cursor_ms(), 5000);
        r.scrub_to(-5);
        assert_eq!(r.cursor_ms(), 1000);
    }
}
