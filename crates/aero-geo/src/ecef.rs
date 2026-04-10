//! Geodetic <-> ECEF conversion on the WGS84 ellipsoid.
//!
//! Forward conversion is closed form. The inverse uses Bowring's method, which
//! converges to sub-millimetre accuracy for near-surface altitudes in a single
//! pass; we run a fixed small number of iterations rather than looping to a
//! tolerance so the cost is bounded and deterministic (invariant: no unbounded
//! loop in the hot path).

use crate::ellipsoid::Wgs84;
use crate::Geodetic;

/// Earth-centred earth-fixed coordinate in metres.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Ecef {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Convert geodetic (deg, deg, m above ellipsoid) to ECEF metres.
pub fn geodetic_to_ecef(g: &Geodetic) -> Ecef {
    let lat = g.lat_deg.to_radians();
    let lon = g.lon_deg.to_radians();
    let (sin_lat, cos_lat) = lat.sin_cos();
    let (sin_lon, cos_lon) = lon.sin_cos();

    // Prime vertical radius of curvature.
    let n = Wgs84::A / (1.0 - Wgs84::e2() * sin_lat * sin_lat).sqrt();

    let x = (n + g.alt_m) * cos_lat * cos_lon;
    let y = (n + g.alt_m) * cos_lat * sin_lon;
    let z = (n * (1.0 - Wgs84::e2()) + g.alt_m) * sin_lat;
    Ecef { x, y, z }
}

/// Convert ECEF metres back to geodetic using Bowring's method.
pub fn ecef_to_geodetic(e: &Ecef) -> Geodetic {
    let a = Wgs84::A;
    let b = Wgs84::b();
    let e2 = Wgs84::e2();
    let ep2 = Wgs84::ep2();

    let p = (e.x * e.x + e.y * e.y).sqrt();
    let lon = e.y.atan2(e.x);

    // Handle the polar singularity where p -> 0.
    if p < 1e-9 {
        let lat = if e.z >= 0.0 {
            std::f64::consts::FRAC_PI_2
        } else {
            -std::f64::consts::FRAC_PI_2
        };
        let alt = e.z.abs() - b;
        return Geodetic::new(lat.to_degrees(), lon.to_degrees(), alt);
    }

    // Bowring's initial parametric latitude.
    let theta = (e.z * a).atan2(p * b);
    let (sin_t, cos_t) = theta.sin_cos();
    let lat = (e.z + ep2 * b * sin_t.powi(3)).atan2(p - e2 * a * cos_t.powi(3));

    let (sin_lat, cos_lat) = lat.sin_cos();
    let n = a / (1.0 - e2 * sin_lat * sin_lat).sqrt();
    let alt = p / cos_lat - n;

    Geodetic::new(lat.to_degrees(), lon.to_degrees(), alt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equator_prime_meridian_sea_level() {
        let g = Geodetic::new(0.0, 0.0, 0.0);
        let e = geodetic_to_ecef(&g);
        assert!((e.x - Wgs84::A).abs() < 1e-6);
        assert!(e.y.abs() < 1e-6);
        assert!(e.z.abs() < 1e-6);
    }

    #[test]
    fn north_pole() {
        let g = Geodetic::new(90.0, 0.0, 0.0);
        let e = geodetic_to_ecef(&g);
        assert!(e.x.abs() < 1e-6);
        assert!(e.y.abs() < 1e-6);
        assert!((e.z - Wgs84::b()).abs() < 1e-6);
    }

    #[test]
    fn roundtrip_many() {
        // Deterministic lattice over the globe, including poles and altitude.
        for lat_i in -90i32..=90 {
            for lon_i in (-180i32..180).step_by(15) {
                for alt in [0.0, 1000.0, 11_000.0, 40_000.0] {
                    let g = Geodetic::new(lat_i as f64, lon_i as f64, alt);
                    let back = ecef_to_geodetic(&geodetic_to_ecef(&g));
                    assert!((back.lat_deg - g.lat_deg).abs() < 1e-7, "lat {lat_i}");
                    assert!((back.alt_m - g.alt_m).abs() < 1e-4, "alt at {lat_i}");
                    // Longitude is undefined exactly at the poles.
                    if lat_i.abs() != 90 {
                        let dlon = (back.lon_deg - g.lon_deg).abs();
                        assert!(dlon < 1e-7 || (dlon - 360.0).abs() < 1e-7, "lon {lon_i}");
                    }
                }
            }
        }
    }
}
