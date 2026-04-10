//! Units and the altitude-datum distinction.
//!
//! Aviation feeds mix feet and metres, knots and m/s, and two unrelated
//! altitude references. The honesty invariants forbid fusing altitude datums by
//! averaging, so the two altitudes are different types and conversions are
//! named explicitly.

use serde::{Deserialize, Serialize};

const FEET_PER_METRE: f64 = 3.280_839_895;
const KNOTS_PER_MPS: f64 = 1.943_844_492;

/// Barometric altitude: pressure altitude referenced to the 1013.25 hPa
/// standard datum. This is what most Mode S transponders report. It is NOT
/// height above the ellipsoid and must not be used directly for ECEF placement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BaroAltM(pub f64);

/// Geometric altitude: height above the WGS84 ellipsoid in metres. This is the
/// value `aero-geo` expects for placing an aircraft in ECEF.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoAltM(pub f64);

impl BaroAltM {
    pub fn from_feet(ft: f64) -> Self {
        Self(ft / FEET_PER_METRE)
    }
    pub fn to_feet(self) -> f64 {
        self.0 * FEET_PER_METRE
    }
}

impl GeoAltM {
    pub fn from_feet(ft: f64) -> Self {
        Self(ft / FEET_PER_METRE)
    }
    pub fn to_feet(self) -> f64 {
        self.0 * FEET_PER_METRE
    }
}

/// Ground speed in metres per second.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpeedMps(pub f64);

impl SpeedMps {
    pub fn from_knots(kt: f64) -> Self {
        Self(kt / KNOTS_PER_MPS)
    }
    pub fn to_knots(self) -> f64 {
        self.0 * KNOTS_PER_MPS
    }
}

/// Vertical rate in metres per second; positive is climbing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VerticalRateMps(pub f64);

impl VerticalRateMps {
    pub fn from_fpm(fpm: f64) -> Self {
        Self(fpm / FEET_PER_METRE / 60.0)
    }
    pub fn to_fpm(self) -> f64 {
        self.0 * FEET_PER_METRE * 60.0
    }
}

/// True track (direction of travel over ground) versus heading (where the nose
/// points). These differ in wind and are kept apart on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrueTrackDeg(pub f64);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeadingDeg(pub f64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feet_metre_roundtrip() {
        let a = BaroAltM::from_feet(35_000.0);
        assert!((a.to_feet() - 35_000.0).abs() < 1e-6);
        // 35000 ft is about 10668 m.
        assert!((a.0 - 10_668.0).abs() < 1.0, "baro m = {}", a.0);
    }

    #[test]
    fn knots_roundtrip() {
        let s = SpeedMps::from_knots(450.0);
        assert!((s.to_knots() - 450.0).abs() < 1e-6);
        assert!((s.0 - 231.5).abs() < 0.5, "mps = {}", s.0);
    }

    #[test]
    fn vertical_rate_sign_and_scale() {
        let vr = VerticalRateMps::from_fpm(-1800.0);
        assert!(vr.0 < 0.0);
        assert!((vr.to_fpm() - (-1800.0)).abs() < 1e-6);
    }

    #[test]
    fn baro_and_geo_are_distinct_types() {
        // This test documents intent: the two altitudes cannot be mixed up by
        // accident because they are different types with no cross conversion.
        let b = BaroAltM(10_000.0);
        let g = GeoAltM(10_050.0);
        assert_ne!(b.0, g.0);
    }
}
