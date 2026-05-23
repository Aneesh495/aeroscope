//! Viewport definition and containment, dateline- and pole-aware (A09).
//!
//! A viewport is a latitude band plus a longitude span and an optional altitude
//! band. The longitude span is stored as (west, east) and may wrap the
//! antimeridian (west > east means the span crosses +/-180). Containment then
//! has two cases, which is exactly where naive code drops or duplicates tracks
//! near the dateline.

use aero_geo::{normalize_lon_deg, Geodetic};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub south_lat: f64,
    pub north_lat: f64,
    /// Western edge longitude, normalized.
    pub west_lon: f64,
    /// Eastern edge longitude, normalized. If west > east the span wraps the
    /// antimeridian.
    pub east_lon: f64,
    /// Optional altitude band in metres (geometric). None means any altitude.
    pub min_alt_m: Option<f64>,
    pub max_alt_m: Option<f64>,
}

impl Viewport {
    pub fn new(south_lat: f64, north_lat: f64, west_lon: f64, east_lon: f64) -> Self {
        Self {
            south_lat,
            north_lat,
            west_lon: normalize_lon_deg(west_lon),
            east_lon: normalize_lon_deg(east_lon),
            min_alt_m: None,
            max_alt_m: None,
        }
    }

    pub fn with_altitude(mut self, min_m: f64, max_m: f64) -> Self {
        self.min_alt_m = Some(min_m);
        self.max_alt_m = Some(max_m);
        self
    }

    /// True if the viewport longitude span wraps the antimeridian.
    pub fn wraps_antimeridian(&self) -> bool {
        self.west_lon > self.east_lon
    }

    /// Does this viewport contain the given position?
    pub fn contains(&self, p: &Geodetic) -> bool {
        if p.lat_deg < self.south_lat || p.lat_deg > self.north_lat {
            return false;
        }
        let lon_ok = if self.wraps_antimeridian() {
            // Span crosses the dateline: inside if east of west OR west of east.
            p.lon_deg >= self.west_lon || p.lon_deg <= self.east_lon
        } else {
            p.lon_deg >= self.west_lon && p.lon_deg <= self.east_lon
        };
        if !lon_ok {
            return false;
        }
        if let (Some(lo), Some(hi)) = (self.min_alt_m, self.max_alt_m) {
            if p.alt_m < lo || p.alt_m > hi {
                return false;
            }
        }
        true
    }

    /// Which 1-degree longitude columns this viewport touches, used to pick
    /// grid cells. Handles the wrap by emitting two ranges.
    pub fn lon_columns(&self) -> Vec<i32> {
        let w = self.west_lon.floor() as i32;
        let e = self.east_lon.floor() as i32;
        let mut cols = Vec::new();
        if self.wraps_antimeridian() {
            for c in w..180 {
                cols.push(c);
            }
            for c in -180..=e {
                cols.push(c);
            }
        } else {
            for c in w..=e {
                cols.push(c);
            }
        }
        cols
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_viewport_contains_interior_rejects_exterior() {
        let v = Viewport::new(0.0, 10.0, 0.0, 10.0);
        assert!(v.contains(&Geodetic::new(5.0, 5.0, 0.0)));
        assert!(!v.contains(&Geodetic::new(11.0, 5.0, 0.0)));
        assert!(!v.contains(&Geodetic::new(5.0, 11.0, 0.0)));
    }

    #[test]
    fn wrapping_viewport_contains_both_sides_of_dateline() {
        // From 170E eastward across 180 to 170W (i.e. -170).
        let v = Viewport::new(0.0, 20.0, 170.0, -170.0);
        assert!(v.wraps_antimeridian());
        assert!(v.contains(&Geodetic::new(10.0, 175.0, 0.0)));
        assert!(v.contains(&Geodetic::new(10.0, -175.0, 0.0)));
        // A point in the middle of the opposite side is excluded.
        assert!(!v.contains(&Geodetic::new(10.0, 0.0, 0.0)));
    }

    #[test]
    fn altitude_band_filters() {
        let v = Viewport::new(0.0, 10.0, 0.0, 10.0).with_altitude(5000.0, 12000.0);
        assert!(v.contains(&Geodetic::new(5.0, 5.0, 10000.0)));
        assert!(!v.contains(&Geodetic::new(5.0, 5.0, 1000.0)));
        assert!(!v.contains(&Geodetic::new(5.0, 5.0, 20000.0)));
    }

    #[test]
    fn lon_columns_wrap() {
        let v = Viewport::new(0.0, 10.0, 178.0, -178.0);
        let cols = v.lon_columns();
        assert!(cols.contains(&178));
        assert!(cols.contains(&179));
        assert!(cols.contains(&-180));
        assert!(cols.contains(&-179));
        assert!(!cols.contains(&0));
    }
}
