//! Spatial indexing and viewport subscription support (A09).
//!
//! The grid index answers "which aircraft are in this viewport" in time
//! proportional to the viewed area, and the viewport type handles the hard
//! geographic cases (antimeridian wrap, poles, altitude bands) so a track near
//! the dateline is never dropped or duplicated.

pub mod grid;
pub mod viewport;

pub use grid::{SpatialGrid, TrackKey};
pub use viewport::Viewport;
