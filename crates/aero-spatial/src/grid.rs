//! A coarse latitude/longitude grid index over aircraft positions (A09).
//!
//! Each aircraft lives in exactly one 1-degree cell keyed by (lat_col,
//! lon_col). A viewport query visits only the cells the viewport touches, so
//! cost scales with the viewed area rather than the whole globe. The grid is
//! intentionally simple and uniform; the win is correctness near the poles and
//! dateline, not asymptotic cleverness. For the globe scale we care about
//! (tens of thousands of tracks) a uniform grid with bounded per-cell lists is
//! both fast and easy to reason about.

use crate::viewport::Viewport;
use aero_geo::Geodetic;
use std::collections::HashMap;

/// Key identifying an aircraft in the index.
pub type TrackKey = String; // ICAO24 hex

#[derive(Debug, Clone, Copy)]
struct Entry {
    pos: Geodetic,
}

#[derive(Default)]
pub struct SpatialGrid {
    /// cell (lat_col, lon_col) -> set of track keys.
    cells: HashMap<(i32, i32), Vec<TrackKey>>,
    /// track key -> its current position and cell, for O(1) move/remove.
    positions: HashMap<TrackKey, (Entry, (i32, i32))>,
}

fn cell_of(p: &Geodetic) -> (i32, i32) {
    // Clamp latitude so the poles land in valid rows; longitude is already
    // normalized to [-180,180) by Geodetic.
    let lat = p.lat_deg.clamp(-90.0, 89.999);
    (lat.floor() as i32, p.lon_deg.floor() as i32)
}

impl SpatialGrid {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or move a track to a new position.
    pub fn upsert(&mut self, key: TrackKey, pos: Geodetic) {
        let new_cell = cell_of(&pos);
        if let Some((_, old_cell)) = self.positions.get(&key).copied() {
            if old_cell != new_cell {
                if let Some(v) = self.cells.get_mut(&old_cell) {
                    v.retain(|k| k != &key);
                    if v.is_empty() {
                        self.cells.remove(&old_cell);
                    }
                }
                self.cells.entry(new_cell).or_default().push(key.clone());
            }
        } else {
            self.cells.entry(new_cell).or_default().push(key.clone());
        }
        self.positions.insert(key, (Entry { pos }, new_cell));
    }

    pub fn remove(&mut self, key: &TrackKey) {
        if let Some((_, cell)) = self.positions.remove(key) {
            if let Some(v) = self.cells.get_mut(&cell) {
                v.retain(|k| k != key);
                if v.is_empty() {
                    self.cells.remove(&cell);
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.positions.len()
    }
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Return all track keys whose position falls inside the viewport. Visits
    /// only the touched cells, then does an exact containment test per entry so
    /// the 1-degree cell granularity never produces a false positive.
    pub fn query(&self, vp: &Viewport) -> Vec<TrackKey> {
        let lat_lo = vp.south_lat.floor() as i32;
        let lat_hi = vp.north_lat.floor() as i32;
        let cols = vp.lon_columns();
        let mut out = Vec::new();
        for lat in lat_lo..=lat_hi {
            for &lon in &cols {
                if let Some(keys) = self.cells.get(&(lat, lon)) {
                    for k in keys {
                        if let Some((e, _)) = self.positions.get(k) {
                            if vp.contains(&e.pos) {
                                out.push(k.clone());
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_query_remove() {
        let mut g = SpatialGrid::new();
        g.upsert("a".into(), Geodetic::new(5.0, 5.0, 0.0));
        g.upsert("b".into(), Geodetic::new(50.0, 50.0, 0.0));
        let vp = Viewport::new(0.0, 10.0, 0.0, 10.0);
        let r = g.query(&vp);
        assert_eq!(r, vec!["a".to_string()]);
        g.remove(&"a".into());
        assert!(g.query(&vp).is_empty());
    }

    #[test]
    fn move_updates_cell() {
        let mut g = SpatialGrid::new();
        g.upsert("a".into(), Geodetic::new(5.0, 5.0, 0.0));
        // Move far away.
        g.upsert("a".into(), Geodetic::new(50.0, 50.0, 0.0));
        assert_eq!(g.len(), 1);
        assert!(g.query(&Viewport::new(0.0, 10.0, 0.0, 10.0)).is_empty());
        assert_eq!(g.query(&Viewport::new(45.0, 55.0, 45.0, 55.0)).len(), 1);
    }

    #[test]
    fn dateline_query_finds_both_sides() {
        let mut g = SpatialGrid::new();
        g.upsert("east".into(), Geodetic::new(10.0, 179.0, 0.0));
        g.upsert("west".into(), Geodetic::new(10.0, -179.0, 0.0));
        g.upsert("far".into(), Geodetic::new(10.0, 0.0, 0.0));
        let vp = Viewport::new(0.0, 20.0, 178.0, -178.0);
        let mut r = g.query(&vp);
        r.sort();
        assert_eq!(r, vec!["east".to_string(), "west".to_string()]);
    }

    #[test]
    fn polar_position_indexable() {
        let mut g = SpatialGrid::new();
        g.upsert("np".into(), Geodetic::new(89.9, 0.0, 0.0));
        let vp = Viewport::new(89.0, 90.0, -1.0, 1.0);
        assert_eq!(g.query(&vp), vec!["np".to_string()]);
    }

    #[test]
    fn no_duplicate_tracks_in_query() {
        let mut g = SpatialGrid::new();
        g.upsert("a".into(), Geodetic::new(5.0, 5.0, 0.0));
        let r = g.query(&Viewport::new(0.0, 10.0, 0.0, 10.0));
        assert_eq!(r.len(), 1); // appears exactly once
    }
}
