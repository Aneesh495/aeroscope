//! Independent geographic recomputation for the verifier (G02, G13).
//!
//! This is deliberately a SECOND implementation of the geodesy used elsewhere,
//! written from a different formulation, so comparing it against `aero-geo`
//! is a genuine differential test rather than a tautology. If both agree to a
//! metre-level tolerance across many cases, a transcription error in either is
//! very unlikely to survive. The verifier never imports aero-geo's distance for
//! its decisive checks; it uses these functions.

/// WGS84 constants restated here independently (same physical datum).
const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_223_563;

/// Spherical-law-of-cosines great-circle distance, metres. A different formula
/// from aero-geo's haversine; both approximate the sphere but via different
/// trig, so agreement cross-checks the arithmetic. Guards the acos domain.
pub fn distance_cosine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6_371_008.8;
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dlon = (lon2 - lon1).to_radians();
    let c = (p1.sin() * p2.sin() + p1.cos() * p2.cos() * dlon.cos()).clamp(-1.0, 1.0);
    R * c.acos()
}

/// Geodetic to ECEF, independent transcription (metres, geometric altitude).
pub fn geodetic_to_ecef(lat_deg: f64, lon_deg: f64, alt_m: f64) -> (f64, f64, f64) {
    let e2 = F * (2.0 - F);
    let lat = lat_deg.to_radians();
    let lon = lon_deg.to_radians();
    let n = A / (1.0 - e2 * lat.sin().powi(2)).sqrt();
    let x = (n + alt_m) * lat.cos() * lon.cos();
    let y = (n + alt_m) * lat.cos() * lon.sin();
    let z = (n * (1.0 - e2) + alt_m) * lat.sin();
    (x, y, z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geo::{geodetic_to_ecef as ag_ecef, haversine_m, Geodetic};

    #[test]
    fn distance_agrees_with_aero_geo_across_many_cases() {
        // Deterministic lattice; compare the two independent distance formulas.
        let mut max_rel = 0.0f64;
        let mut count = 0u32;
        for la1 in (-80..=80).step_by(20) {
            for lo1 in (-170..=170).step_by(40) {
                for la2 in (-80..=80).step_by(20) {
                    for lo2 in (-170..=170).step_by(40) {
                        let a = Geodetic::new(la1 as f64, lo1 as f64, 0.0);
                        let b = Geodetic::new(la2 as f64, lo2 as f64, 0.0);
                        let d1 = haversine_m(&a, &b);
                        let d2 = distance_cosine_m(la1 as f64, lo1 as f64, la2 as f64, lo2 as f64);
                        if d1 > 1000.0 {
                            max_rel = max_rel.max((d1 - d2).abs() / d1);
                            count += 1;
                        }
                    }
                }
            }
        }
        assert!(count > 1000, "expected a large case count, got {count}");
        // Two spherical formulas should agree very tightly.
        assert!(max_rel < 1e-6, "max relative distance disagreement {max_rel}");
    }

    #[test]
    fn ecef_agrees_with_aero_geo() {
        let mut max_m = 0.0f64;
        for la in (-90..=90).step_by(10) {
            for lo in (-180..180).step_by(30) {
                for alt in [0.0, 10_000.0, 40_000.0] {
                    let (x, y, z) = geodetic_to_ecef(la as f64, lo as f64, alt);
                    let e = ag_ecef(&Geodetic::new(la as f64, lo as f64, alt));
                    let d = ((x - e.x).powi(2) + (y - e.y).powi(2) + (z - e.z).powi(2)).sqrt();
                    max_m = max_m.max(d);
                }
            }
        }
        assert!(max_m < 1e-3, "max ECEF disagreement {max_m} m");
    }
}
