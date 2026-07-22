//! Aeroscope server library: the integrated pipeline that ties the backend
//! crates together. The binary (`src/main.rs`) is a thin command wrapper over
//! this, and integration tests drive `pipeline::Pipeline` directly.

pub mod pipeline;

pub use pipeline::{Pipeline, PipelineMetrics};
