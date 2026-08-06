//! The stored log record and its deduplication key.

use aero_types::Observation;
use serde::{Deserialize, Serialize};

/// Deduplication key for idempotent ingestion. Two observations are "the same"
/// if they come from the same source, for the same aircraft, at the same event
/// time measured on the same clock basis. We include the basis so an
/// observation-time record and an ingest-fallback record at the same numeric
/// millisecond are not wrongly merged.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DedupKey {
    pub source: String,
    pub icao24: [u8; 3],
    pub event_time_ms: i64,
    pub basis: u8,
}

/// An observation as stored in the log, with the schema version it was written
/// under so a future format change can be migrated rather than misread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggedObservation {
    /// Log record schema version. Bumped on any breaking change to the frame
    /// payload shape.
    pub schema_version: u16,
    pub observation: Observation,
}

impl LoggedObservation {
    pub const CURRENT_SCHEMA: u16 = 1;

    pub fn new(observation: Observation) -> Self {
        Self {
            schema_version: Self::CURRENT_SCHEMA,
            observation,
        }
    }

    pub fn dedup_key(&self) -> DedupKey {
        let (event_time_ms, basis) = self.observation.effective_event_time_ms();
        DedupKey {
            source: self.observation.source.0.clone(),
            icao24: self.observation.icao24.0,
            event_time_ms,
            basis: basis as u8,
        }
    }
}
