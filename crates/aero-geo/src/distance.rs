//! Distances, bearings, and great-circle interpolation.
//!
//! Two distance methods are provided on purpose: `haversine_m` (spherical, fast,
//! ~0.3% worst-case error) and `vincenty_m` (ellipsoidal, sub-millimetre but
//! iterative). The streaming path uses haversine; the G02 oracle uses vincenty.
//! Having both lets a differential test catch a regression in either one.

use crate::ellipsoid::Wgs84;
use crate::Geodetic;

/// Mean Earth radius for the spherical model, metres (IUGG R1).
const R_MEAN_M: f64 = 6_371_008.8;

/// Great-circle distance on a sphere, metres. Numerically stable near zero
/// separation because it uses atan2 rather than acos.
pub fn haversine_m(a: &Geodetic, b: &Geodetic) -> f64 {
    let lat1 = a.lat_deg.to_radians();
    let lat2 = b.lat_deg.to_radians();
    let dlat = (b.lat_deg - a.lat_deg).to_radians();
    let dlon = (b.lon_deg - a.lon_deg).to_radians();

    let sin_dlat = (dlat / 2.0).sin();
    let sin_dlon = (dlon / 2.0).sin();
    let h = sin_dlat * sin_dlat + lat1.cos() * lat2.cos() * sin_dlon * sin_dlon;
    2.0 * R_MEAN_M * h.sqrt().clamp(0.0, 1.0).asin()
}

/// Vincenty inverse-formula geodesic distance on the WGS84 ellipsoid, metres.
/// Returns `None` if the iteration fails to converge (near-antipodal points),
/// which the caller must handle rather than trust a divergent value.
pub fn vincenty_m(a: &Geodetic, b: &Geodetic) -> Option<f64> {
    let f = Wgs84::f();
    let bb = Wgs84::b();

    let l = (b.lon_deg - a.lon_deg).to_radians();
    let u1 = ((1.0 - f) * a.lat_deg.to_radians().tan()).atan();
    let u2 = ((1.0 - f) * b.lat_deg.to_radians().tan()).atan();
    let (sin_u1, cos_u1) = u1.sin_cos();
    let (sin_u2, cos_u2) = u2.sin_cos();

    let mut lambda = l;
    let mut sin_sigma;
    let mut cos_sigma;
    let mut sigma;
    let mut cos_sq_alpha;
    let mut cos_2sigma_m;

    // Bounded iteration: Vincenty converges in a handful of steps for
    // non-antipodal pairs. Cap at 200 and report non-convergence.
    for _ in 0..200 {
        let (sin_lambda, cos_lambda) = lambda.sin_cos();
        sin_sigma = ((cos_u2 * sin_lambda).powi(2)
            + (cos_u1 * sin_u2 - sin_u1 * cos_u2 * cos_lambda).powi(2))
        .sqrt();
        if sin_sigma == 0.0 {
            return Some(0.0); // coincident points
        }
        cos_sigma = sin_u1 * sin_u2 + cos_u1 * cos_u2 * cos_lambda;
        sigma = sin_sigma.atan2(cos_sigma);
        let sin_alpha = cos_u1 * cos_u2 * sin_lambda / sin_sigma;
        cos_sq_alpha = 1.0 - sin_alpha * sin_alpha;
        cos_2sigma_m = if cos_sq_alpha != 0.0 {
            cos_sigma - 2.0 * sin_u1 * sin_u2 / cos_sq_alpha
        } else {
            0.0 // equatorial line
        };
        let c = f / 16.0 * cos_sq_alpha * (4.0 + f * (4.0 - 3.0 * cos_sq_alpha));
        let lambda_prev = lambda;
        lambda = l
            + (1.0 - c)
                * f
                * sin_alpha
                * (sigma
                    + c * sin_sigma
                        * (cos_2sigma_m
                            + c * cos_sigma * (-1.0 + 2.0 * cos_2sigma_m * cos_2sigma_m)));
        if (lambda - lambda_prev).abs() < 1e-12 {
            let u_sq = cos_sq_alpha * Wgs84::ep2();
            let big_a = 1.0
                + u_sq / 16384.0 * (4096.0 + u_sq * (-768.0 + u_sq * (320.0 - 175.0 * u_sq)));
            let big_b = u_sq / 1024.0 * (256.0 + u_sq * (-128.0 + u_sq * (74.0 - 47.0 * u_sq)));
            let delta_sigma = big_b
                * sin_sigma
                * (cos_2sigma_m
                    + big_b / 4.0
                        * (cos_sigma * (-1.0 + 2.0 * cos_2sigma_m * cos_2sigma_m)
                            - big_b / 6.0
                                * cos_2sigma_m
                                * (-3.0 + 4.0 * sin_sigma * sin_sigma)
                                * (-3.0 + 4.0 * cos_2sigma_m * cos_2sigma_m)));
            return Some(bb * big_a * (sigma - delta_sigma));
        }
    }
    None
}

/// Initial bearing (forward azimuth) from `a` to `b`, degrees in [0,360).
pub fn initial_bearing_deg(a: &Geodetic, b: &Geodetic) -> f64 {
    let lat1 = a.lat_deg.to_radians();
    let lat2 = b.lat_deg.to_radians();
    let dlon = (b.lon_deg - a.lon_deg).to_radians();
    let y = dlon.sin() * lat2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlon.cos();
    let brng = y.atan2(x).to_degrees();
    (brng + 360.0) % 360.0
}

/// Great-circle interpolation. `t` in [0,1] returns the point a fraction `t`
/// along the shorter geodesic from `a` to `b` (spherical model). Used for the
/// interpolated track-lifecycle state; the error bound is the haversine error.
pub fn interpolate_great_circle(a: &Geodetic, b: &Geodetic, t: f64) -> Geodetic {
    let lat1 = a.lat_deg.to_radians();
    let lon1 = a.lon_deg.to_radians();
    let lat2 = b.lat_deg.to_radians();
    let lon2 = b.lon_deg.to_radians();

    let d = haversine_m(a, b) / R_MEAN_M; // angular distance
    if d < 1e-12 {
        return *a;
    }
    let sin_d = d.sin();
    let aa = ((1.0 - t) * d).sin() / sin_d;
    let bb = (t * d).sin() / sin_d;

    let x = aa * lat1.cos() * lon1.cos() + bb * lat2.cos() * lon2.cos();
    let y = aa * lat1.cos() * lon1.sin() + bb * lat2.cos() * lon2.sin();
    let z = aa * lat1.sin() + bb * lat2.sin();

    let lat = z.atan2((x * x + y * y).sqrt());
    let lon = y.atan2(x);
    let alt = a.alt_m + (b.alt_m - a.alt_m) * t;
    Geodetic::new(lat.to_degrees(), lon.to_degrees(), alt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_vincenty_agree_short_range() {
        // Within a few hundred km the two methods should agree to ~0.5%.
        let a = Geodetic::new(37.62, -122.38, 0.0); // ~SFO
        let b = Geodetic::new(33.94, -118.41, 0.0); // ~LAX
        let hv = haversine_m(&a, &b);
        let vn = vincenty_m(&a, &b).unwrap();
        let rel = (hv - vn).abs() / vn;
        assert!(rel < 0.005, "rel error {rel}");
        // Known distance SFO-LAX ~ 543 km.
        assert!((vn - 543_000.0).abs() < 10_000.0, "vincenty {vn}");
    }

    #[test]
    fn bearing_due_north_and_east() {
        let a = Geodetic::new(0.0, 0.0, 0.0);
        let north = Geodetic::new(10.0, 0.0, 0.0);
        assert!((initial_bearing_deg(&a, &north) - 0.0).abs() < 1e-6);
        let east = Geodetic::new(0.0, 10.0, 0.0);
        assert!((initial_bearing_deg(&a, &east) - 90.0).abs() < 1e-6);
    }

    #[test]
    fn interpolation_midpoint_on_path() {
        let a = Geodetic::new(0.0, 0.0, 0.0);
        let b = Geodetic::new(0.0, 10.0, 0.0);
        let mid = interpolate_great_circle(&a, &b, 0.5);
        assert!((mid.lat_deg - 0.0).abs() < 1e-9);
        assert!((mid.lon_deg - 5.0).abs() < 1e-6);
    }
}
