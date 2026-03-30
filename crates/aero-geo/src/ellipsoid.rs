//! WGS84 ellipsoid constants.
//!
//! These are the defining parameters of the WGS84 datum. The semi-major axis
//! `a` and inverse flattening `1/f` are exact definitions; everything else is
//! derived so there is a single source of truth and no copy error can creep in.

#[derive(Debug, Clone, Copy)]
pub struct Wgs84;

impl Wgs84 {
    /// Semi-major axis (equatorial radius), metres. Defining constant.
    pub const A: f64 = 6_378_137.0;
    /// Inverse flattening. Defining constant.
    pub const INV_F: f64 = 298.257_223_563;

    /// Flattening f = 1 / (1/f).
    pub const fn f() -> f64 {
        1.0 / Self::INV_F
    }

    /// Semi-minor axis b = a * (1 - f), metres.
    pub fn b() -> f64 {
        Self::A * (1.0 - Self::f())
    }

    /// First eccentricity squared e^2 = f * (2 - f).
    pub fn e2() -> f64 {
        let f = Self::f();
        f * (2.0 - f)
    }

    /// Second eccentricity squared e'^2 = e^2 / (1 - e^2).
    pub fn ep2() -> f64 {
        Self::e2() / (1.0 - Self::e2())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_constants_match_published_values() {
        // b ~= 6356752.3142 m
        assert!((Wgs84::b() - 6_356_752.314_2).abs() < 1e-3, "b = {}", Wgs84::b());
        // e^2 ~= 0.00669437999014
        assert!((Wgs84::e2() - 0.006_694_379_990_14).abs() < 1e-12, "e2 = {}", Wgs84::e2());
    }
}
