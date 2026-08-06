//! Geographic reference computations for Aeroscope.
//!
//! This crate intentionally depends on no external geodesy library. It is the
//! independent reference oracle used by gate G02: its results are compared
//! against the rest of the system, so it must be derived from first principles
//! and documented, not borrowed from a crate whose internals we cannot audit.
//!
//! Units and frames (see `docs/SPECIFICATION.md`):
//! - Latitude/longitude are WGS84 geodetic degrees unless a type says otherwise.
//! - Longitude is normalized to [-180, 180).
//! - ECEF coordinates are metres in the WGS84 earth-centred earth-fixed frame.
//! - Altitude for ECEF conversion is geometric height above the WGS84
//!   ellipsoid in metres. Barometric altitude is a different quantity and is
//!   never silently substituted here (see `aero-types`).

pub mod antimeridian;
pub mod distance;
pub mod ecef;
pub mod ellipsoid;

pub use antimeridian::{crosses_antimeridian, normalize_lon_deg, split_at_antimeridian};
pub use distance::{haversine_m, initial_bearing_deg, interpolate_great_circle, vincenty_m};
pub use ecef::{ecef_to_geodetic, geodetic_to_ecef, Ecef};
pub use ellipsoid::Wgs84;

/// A geodetic position. Longitude normalized to [-180,180), latitude in
/// [-90,90]. `alt_m` is geometric height above the ellipsoid in metres.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Geodetic {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub alt_m: f64,
}

impl Geodetic {
    pub fn new(lat_deg: f64, lon_deg: f64, alt_m: f64) -> Self {
        Self {
            lat_deg,
            lon_deg: normalize_lon_deg(lon_deg),
            alt_m,
        }
    }

    /// Returns true if the coordinate is a physically valid geodetic position.
    pub fn is_valid(&self) -> bool {
        self.lat_deg.is_finite()
            && self.lon_deg.is_finite()
            && self.alt_m.is_finite()
            && (-90.0..=90.0).contains(&self.lat_deg)
            && (-180.0..180.0).contains(&self.lon_deg)
    }
}
