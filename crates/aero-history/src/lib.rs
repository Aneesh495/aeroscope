//! Historical storage, time-range queries, path simplification, and replay
//! (A12, A13).
//!
//! History is organized into time partitions so a range query touches only the
//! relevant partitions. Each stored point keeps its event time and track state,
//! so a gap or a stale interval reconstructs exactly rather than appearing as a
//! continuous observed path. Path simplification carries a declared metre-level
//! error bound, and the replay controller is deterministic and never presents
//! itself as live.

pub mod replay;
pub mod simplify;

pub use replay::{PlayState, ReplayController};
pub use simplify::{max_deviation_m, simplify_indices};

use aero_geo::Geodetic;
use std::collections::BTreeMap;

/// One stored historical point for a track.
#[derive(Debug, Clone, PartialEq)]
pub struct HistPoint {
    pub event_time_ms: i64,
    pub position: Option<Geodetic>,
    /// Whether this point was an actually-observed state (true) or a gap/stale
    /// marker (false), so replay never paints a gap as a solid observed path.
    pub observed: bool,
}

/// Partition width in milliseconds. One hour keeps partitions small enough to
/// prune aggressively and large enough to avoid tiny-file churn.
pub const PARTITION_MS: i64 = 3_600_000;

fn partition_of(event_ms: i64) -> i64 {
    event_ms.div_euclid(PARTITION_MS)
}

/// In-memory partitioned history for one process. A persistent backend would
/// map each partition to a file or a PostGIS partition; the query semantics are
/// identical and tested here.
#[derive(Default)]
pub struct HistoryStore {
    /// track hex -> partition -> ordered points by event time.
    tracks: std::collections::HashMap<String, BTreeMap<i64, Vec<HistPoint>>>,
}

impl HistoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, track: &str, point: HistPoint) {
        let part = partition_of(point.event_time_ms);
        let parts = self.tracks.entry(track.to_string()).or_default();
        let bucket = parts.entry(part).or_default();
        // Keep each partition ordered by event time for range scans.
        let pos = bucket.partition_point(|p| p.event_time_ms <= point.event_time_ms);
        bucket.insert(pos, point);
    }

    /// Bounded time-range query (inclusive). Visits only the partitions the
    /// range overlaps, then filters within them.
    pub fn range(&self, track: &str, from_ms: i64, to_ms: i64) -> Vec<HistPoint> {
        let parts = match self.tracks.get(track) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let lo = partition_of(from_ms);
        let hi = partition_of(to_ms);
        let mut out = Vec::new();
        for (_, bucket) in parts.range(lo..=hi) {
            for p in bucket {
                if p.event_time_ms >= from_ms && p.event_time_ms <= to_ms {
                    out.push(p.clone());
                }
            }
        }
        out
    }

    /// Number of partitions a track occupies, for index inspection.
    pub fn partition_count(&self, track: &str) -> usize {
        self.tracks.get(track).map(|p| p.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(t: i64, observed: bool) -> HistPoint {
        HistPoint {
            event_time_ms: t,
            position: Some(Geodetic::new(0.0, (t / 1000) as f64, 10000.0)),
            observed,
        }
    }

    #[test]
    fn range_query_returns_only_window() {
        let mut h = HistoryStore::new();
        for t in (0..10_000).step_by(1000) {
            h.append("abc", pt(t, true));
        }
        let r = h.range("abc", 3000, 6000);
        assert_eq!(r.len(), 4); // 3000,4000,5000,6000
        assert_eq!(r.first().unwrap().event_time_ms, 3000);
        assert_eq!(r.last().unwrap().event_time_ms, 6000);
    }

    #[test]
    fn partitions_span_hours() {
        let mut h = HistoryStore::new();
        h.append("abc", pt(0, true));
        h.append("abc", pt(PARTITION_MS + 1000, true)); // next hour
        assert_eq!(h.partition_count("abc"), 2);
        // A range across the boundary still returns both.
        let r = h.range("abc", 0, PARTITION_MS + 2000);
        assert_eq!(r.len(), 2);
    }

    #[test]
    fn gap_markers_distinguish_from_observed() {
        let mut h = HistoryStore::new();
        h.append("abc", pt(1000, true));
        h.append("abc", pt(2000, false)); // gap/stale marker
        h.append("abc", pt(3000, true));
        let r = h.range("abc", 0, 5000);
        assert_eq!(r.iter().filter(|p| p.observed).count(), 2);
        assert_eq!(r.iter().filter(|p| !p.observed).count(), 1);
    }

    #[test]
    fn empty_track_returns_nothing() {
        let h = HistoryStore::new();
        assert!(h.range("nope", 0, 1000).is_empty());
    }
}
