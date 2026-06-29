//! Path simplification with a declared geographic error bound (A12).
//!
//! Historical tracks are stored and queried at many zoom levels. We simplify a
//! polyline with Douglas-Peucker using a perpendicular distance measured in
//! metres on the sphere, so the tolerance is a real geographic error the UI can
//! state ("simplified to within 500 m"), not an opaque screen-space epsilon.

use aero_geo::{geodetic_to_ecef, Geodetic};

/// Perpendicular distance in metres from point `p` to the chord `a`-`b`,
/// computed in ECEF so it is a true 3D metre distance independent of latitude.
fn point_to_segment_m(p: &Geodetic, a: &Geodetic, b: &Geodetic) -> f64 {
    let pe = geodetic_to_ecef(p);
    let ae = geodetic_to_ecef(a);
    let be = geodetic_to_ecef(b);
    let (ax, ay, az) = (ae.x, ae.y, ae.z);
    let abx = be.x - ax;
    let aby = be.y - ay;
    let abz = be.z - az;
    let apx = pe.x - ax;
    let apy = pe.y - ay;
    let apz = pe.z - az;
    let ab_len2 = abx * abx + aby * aby + abz * abz;
    if ab_len2 < 1e-9 {
        // Degenerate chord: distance to the point a.
        return (apx * apx + apy * apy + apz * apz).sqrt();
    }
    let t = ((apx * abx + apy * aby + apz * abz) / ab_len2).clamp(0.0, 1.0);
    let cx = ax + t * abx;
    let cy = ay + t * aby;
    let cz = az + t * abz;
    ((pe.x - cx).powi(2) + (pe.y - cy).powi(2) + (pe.z - cz).powi(2)).sqrt()
}

/// Douglas-Peucker simplification keeping all points within `tolerance_m` of the
/// simplified path. Returns indices of retained points so callers can carry
/// associated data (timestamps, states) along.
pub fn simplify_indices(points: &[Geodetic], tolerance_m: f64) -> Vec<usize> {
    let n = points.len();
    if n <= 2 {
        return (0..n).collect();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    // Iterative stack to avoid unbounded recursion (bounded memory invariant).
    let mut stack = vec![(0usize, n - 1)];
    while let Some((lo, hi)) = stack.pop() {
        if hi <= lo + 1 {
            continue;
        }
        let mut max_d = 0.0;
        let mut idx = lo;
        for i in (lo + 1)..hi {
            let d = point_to_segment_m(&points[i], &points[lo], &points[hi]);
            if d > max_d {
                max_d = d;
                idx = i;
            }
        }
        if max_d > tolerance_m {
            keep[idx] = true;
            stack.push((lo, idx));
            stack.push((idx, hi));
        }
    }
    (0..n).filter(|&i| keep[i]).collect()
}

/// The maximum deviation introduced by simplifying to the kept indices, in
/// metres. Used to certify the declared error bound rather than assume it.
pub fn max_deviation_m(points: &[Geodetic], kept: &[usize]) -> f64 {
    if kept.len() < 2 {
        return 0.0;
    }
    let mut max_d = 0.0f64;
    for w in kept.windows(2) {
        let (a, b) = (points[w[0]], points[w[1]]);
        for i in w[0]..=w[1] {
            let d = point_to_segment_m(&points[i], &a, &b);
            max_d = max_d.max(d);
        }
    }
    max_d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_near_straight_segment_collapses_to_endpoints() {
        // A short equatorial run: over ~0.01 deg of arc the great-circle bulge
        // from the chord is sub-metre, so with a 100 m tolerance the intermediate
        // samples collapse. (Over long arcs the equator is a true curve in 3D
        // and intermediate points are correctly kept; that is tested below.)
        let pts: Vec<Geodetic> = (0..=10)
            .map(|i| Geodetic::new(0.0, i as f64 * 0.001, 10000.0))
            .collect();
        let kept = simplify_indices(&pts, 100.0);
        assert_eq!(kept, vec![0, 10]);
    }

    #[test]
    fn long_arc_keeps_intermediate_points_as_true_3d_curve() {
        // 10 deg of equatorial arc bulges ~tens of km from its chord in 3D, so a
        // 100 m tolerance must keep intermediate points. This documents that the
        // metric is a genuine 3D geographic error, not a lat/lon epsilon.
        let pts: Vec<Geodetic> = (0..=10)
            .map(|i| Geodetic::new(0.0, i as f64, 10000.0))
            .collect();
        let kept = simplify_indices(&pts, 100.0);
        assert!(kept.len() > 2, "long arc should retain curvature, got {kept:?}");
    }

    #[test]
    fn deviation_respects_tolerance() {
        // A noisy path; simplification must stay within the declared tolerance.
        let pts: Vec<Geodetic> = (0..=50)
            .map(|i| {
                let x = i as f64 * 0.1;
                let wobble = (i as f64 * 0.7).sin() * 0.001; // ~100 m wobble
                Geodetic::new(wobble, x, 10000.0)
            })
            .collect();
        let tol = 500.0;
        let kept = simplify_indices(&pts, tol);
        let dev = max_deviation_m(&pts, &kept);
        assert!(dev <= tol, "deviation {dev} exceeds tolerance {tol}");
        // And it actually removed something.
        assert!(kept.len() < pts.len());
    }

    #[test]
    fn sharp_corner_is_kept() {
        let pts = vec![
            Geodetic::new(0.0, 0.0, 10000.0),
            Geodetic::new(0.0, 1.0, 10000.0),
            Geodetic::new(1.0, 1.0, 10000.0), // 90-degree turn
        ];
        let kept = simplify_indices(&pts, 100.0);
        assert_eq!(kept, vec![0, 1, 2]);
    }
}
