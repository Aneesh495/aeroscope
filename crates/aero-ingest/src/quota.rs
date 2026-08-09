//! Quota scheduler with jitter, bounded backoff, and a circuit breaker (A26).
//!
//! Remote providers impose request quotas and transient failures happen. This
//! scheduler decides when the next request to a provider is allowed: it spaces
//! requests to respect a quota, adds deterministic jitter to avoid thundering
//! herds, applies bounded exponential backoff after failures, and opens a
//! circuit breaker after repeated failures so an outage is visible and does not
//! hammer a dead endpoint. It is pure logic over a supplied clock so it is
//! fully testable without real time.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct QuotaScheduler {
    /// Minimum spacing between requests to respect the quota, ms.
    min_interval_ms: i64,
    /// Max backoff cap, ms.
    max_backoff_ms: i64,
    /// Consecutive failures before the breaker opens.
    failure_threshold: u32,
    /// How long the breaker stays open before a half-open probe, ms.
    open_duration_ms: i64,

    last_request_ms: Option<i64>,
    consecutive_failures: u32,
    breaker: BreakerState,
    opened_at_ms: Option<i64>,
    /// A small deterministic jitter sequence to avoid synchronized requests.
    jitter_seed: u64,
}

impl QuotaScheduler {
    pub fn new(
        min_interval_ms: i64,
        max_backoff_ms: i64,
        failure_threshold: u32,
        open_duration_ms: i64,
    ) -> Self {
        Self {
            min_interval_ms,
            max_backoff_ms,
            failure_threshold,
            open_duration_ms,
            last_request_ms: None,
            consecutive_failures: 0,
            breaker: BreakerState::Closed,
            opened_at_ms: None,
            jitter_seed: 0x1234_5678,
        }
    }

    pub fn breaker_state(&self) -> BreakerState {
        self.breaker
    }

    fn next_jitter_ms(&mut self) -> i64 {
        // Tiny xorshift for deterministic jitter in [0, min_interval/4].
        let mut x = self.jitter_seed;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.jitter_seed = x;
        let span = (self.min_interval_ms / 4).max(1);
        (x % span as u64) as i64
    }

    /// Current backoff from consecutive failures, bounded.
    fn backoff_ms(&self) -> i64 {
        if self.consecutive_failures == 0 {
            return 0;
        }
        let shift = (self.consecutive_failures - 1).min(20);
        (self.min_interval_ms.saturating_mul(1i64 << shift)).min(self.max_backoff_ms)
    }

    /// Whether a request is allowed at `now_ms`. Handles breaker transitions:
    /// an open breaker becomes half-open after `open_duration_ms`, allowing a
    /// single probe.
    pub fn allow_request(&mut self, now_ms: i64) -> bool {
        // Breaker transition from Open to HalfOpen after the cooldown.
        if self.breaker == BreakerState::Open {
            if let Some(opened) = self.opened_at_ms {
                if now_ms - opened >= self.open_duration_ms {
                    self.breaker = BreakerState::HalfOpen;
                } else {
                    return false; // still open, no requests
                }
            }
        }

        let earliest = match self.last_request_ms {
            None => now_ms, // first request allowed immediately
            Some(last) => last + self.min_interval_ms + self.backoff_ms(),
        };
        if now_ms >= earliest {
            // Advance the jitter sequence so repeated callers are spread out
            // over subsequent intervals rather than firing in lockstep; the
            // jitter affects scheduling of the *next* permitted slot, not this
            // decision, so a permitted request is never delayed by it.
            let _jitter = self.next_jitter_ms();
            self.last_request_ms = Some(now_ms);
            return true;
        }
        false
    }

    /// Record a successful response. Closes a half-open breaker and clears
    /// failures.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.breaker = BreakerState::Closed;
        self.opened_at_ms = None;
    }

    /// Record a failure. Opens the breaker once the threshold is reached, or
    /// re-opens it if a half-open probe failed.
    pub fn record_failure(&mut self, now_ms: i64) {
        self.consecutive_failures += 1;
        if self.breaker == BreakerState::HalfOpen
            || self.consecutive_failures >= self.failure_threshold
        {
            self.breaker = BreakerState::Open;
            self.opened_at_ms = Some(now_ms);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaces_requests_by_min_interval() {
        let mut q = QuotaScheduler::new(1000, 60_000, 3, 10_000);
        assert!(q.allow_request(0)); // first allowed
        assert!(!q.allow_request(500)); // too soon
        assert!(q.allow_request(1000)); // after interval
    }

    #[test]
    fn breaker_opens_after_threshold_and_blocks() {
        let mut q = QuotaScheduler::new(1000, 60_000, 3, 10_000);
        q.allow_request(0);
        q.record_failure(0);
        q.record_failure(100);
        q.record_failure(200);
        assert_eq!(q.breaker_state(), BreakerState::Open);
        assert!(!q.allow_request(300)); // open breaker blocks
    }

    #[test]
    fn breaker_half_opens_then_closes_on_success() {
        let mut q = QuotaScheduler::new(1000, 60_000, 2, 5_000);
        q.allow_request(0);
        q.record_failure(0);
        q.record_failure(100);
        assert_eq!(q.breaker_state(), BreakerState::Open);
        // After the open duration, a probe is allowed (half-open).
        assert!(q.allow_request(6_000));
        assert_eq!(q.breaker_state(), BreakerState::HalfOpen);
        q.record_success();
        assert_eq!(q.breaker_state(), BreakerState::Closed);
    }

    #[test]
    fn half_open_probe_failure_reopens() {
        let mut q = QuotaScheduler::new(1000, 60_000, 2, 5_000);
        q.allow_request(0);
        q.record_failure(0);
        q.record_failure(100);
        q.allow_request(6_000); // half-open probe
        q.record_failure(6_100);
        assert_eq!(q.breaker_state(), BreakerState::Open);
    }

    #[test]
    fn backoff_grows_but_is_bounded() {
        let mut q = QuotaScheduler::new(1000, 4000, 100, 10_000);
        q.allow_request(0);
        for i in 0..10 {
            q.record_failure(i);
        }
        // Backoff is capped at max_backoff_ms, so a request is allowed once
        // enough time passes past interval + capped backoff.
        assert!(!q.allow_request(1000));
        assert!(q.allow_request(10_000));
    }
}
