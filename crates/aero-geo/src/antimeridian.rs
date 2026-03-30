//! Antimeridian (180 deg) handling.
//!
//! The dateline is where naive rendering and spatial queries break: a track
//! that crosses 180 deg either draws a wrong line all the way across the map or
//! disappears from a viewport query. These helpers make the crossing explicit
//! so G02's "no missing or duplicated dateline tracks" requirement is testable.

use crate::Geodetic;

/// Normalize a longitude to the half-open interval [-180, 180).
/// +180 maps to -180 so there is exactly one representation per meridian.
pub fn normalize_lon_deg(lon: f64) -> f64 {
    if !lon.is_finite() {
        return lon;
    }
    let mut x = (lon + 180.0).rem_euclid(360.0) - 180.0;
    if x == 180.0 {
        x = -180.0;
    }
    x
}

/// True if the shorter path between two longitudes crosses the antimeridian.
pub fn crosses_antimeridian(lon_a: f64, lon_b: f64) -> bool {
    (normalize_lon_deg(lon_a) - normalize_lon_deg(lon_b)).abs() > 180.0
}

/// Split a two-point segment that crosses the dateline into two segments,
/// inserting the crossing point at +/-180 with latitude found by linear
/// interpolation in unwrapped longitude. Returns one segment if no crossing.
pub fn split_at_antimeridian(a: &Geodetic, b: &Geodetic) -> Vec<(Geodetic, Geodetic)> {
    if !crosses_antimeridian(a.lon_deg, b.lon_deg) {
        return vec![(*a, *b)];
    }
    // Unwrap b relative to a so the interpolation is monotonic.
    let lon_a = a.lon_deg;
    let mut lon_b = b.lon_deg;
    if lon_b - lon_a > 180.0 {
        lon_b -= 360.0;
    } else if lon_b - lon_a < -180.0 {
        lon_b += 360.0;
    }

    // The crossing meridian in the unwrapped frame.
    let cross_lon = if lon_a < lon_b { 180.0 } else { -180.0 };
    let t = (cross_lon - lon_a) / (lon_b - lon_a);
    let lat_cross = a.lat_deg + (b.lat_deg - a.lat_deg) * t;
    let alt_cross = a.alt_m + (b.alt_m - a.alt_m) * t;

    let cross_near_a = Geodetic::new(lat_cross, cross_lon, alt_cross);
    let cross_near_b = Geodetic::new(lat_cross, -cross_lon, alt_cross);
    vec![(*a, cross_near_a), (cross_near_b, *b)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_wraps() {
        assert_eq!(normalize_lon_deg(190.0), -170.0);
        assert_eq!(normalize_lon_deg(-190.0), 170.0);
        assert_eq!(normalize_lon_deg(180.0), -180.0);
        assert_eq!(normalize_lon_deg(-180.0), -180.0);
        assert_eq!(normalize_lon_deg(0.0), 0.0);
    }

    #[test]
    fn detects_crossing() {
        assert!(crosses_antimeridian(170.0, -170.0));
        assert!(!crosses_antimeridian(170.0, 175.0));
        assert!(!crosses_antimeridian(-10.0, 10.0));
    }

    #[test]
    fn split_produces_two_segments_meeting_at_dateline() {
        let a = Geodetic::new(10.0, 170.0, 10_000.0);
        let b = Geodetic::new(20.0, -170.0, 10_000.0);
        let segs = split_at_antimeridian(&a, &b);
        assert_eq!(segs.len(), 2);
        // First segment ends on +180, second starts on -180.
        assert!((segs[0].1.lon_deg - (-180.0)).abs() < 1e-9 || (segs[0].1.lon_deg - 180.0).abs() < 1e-9);
        // Crossing latitude is the midpoint for this symmetric case (15 deg).
        assert!((segs[0].1.lat_deg - 15.0).abs() < 1e-6, "lat {}", segs[0].1.lat_deg);
    }

    #[test]
    fn no_split_when_not_crossing() {
        let a = Geodetic::new(0.0, 10.0, 0.0);
        let b = Geodetic::new(0.0, 20.0, 0.0);
        assert_eq!(split_at_antimeridian(&a, &b).len(), 1);
    }
}
