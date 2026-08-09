//! Event-time fusion for Aeroscope (A05-A08).
//!
//! The pipeline here takes normalized observations and turns them into fused
//! aircraft states with explicit lifecycle, plausibility screening, conflict
//! retention, and flight-leg segmentation. The defining property is that it
//! never fabricates or averages away genuine signal: conflicting evidence is
//! kept and a declared rule chooses between sources, late events are recorded
//! rather than silently applied, and extrapolation expires at a declared
//! horizon.

pub mod engine;
pub mod geofence;
pub mod lifecycle;
pub mod plausibility;
pub mod segmentation;

pub use engine::{Evidence, FusedState, FusionEngine, FusionOutcome, SelectionReason, SourceTrust};
pub use geofence::{FenceEvent, FenceTracker, Geofence};
pub use lifecycle::{LifecycleConfig, TrackState};
pub use plausibility::{check as check_plausibility, Plausibility};
pub use segmentation::{segment, FlightLeg, LegBoundaryCause, SegmentationConfig};
