//! Error contract for the ingestion and fusion path.
//!
//! Malformed records are quarantined, not dropped silently (A01). The error
//! type carries enough context to record why a record was rejected.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("malformed record: {0}")]
    Malformed(String),

    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(String),

    #[error("field {field} out of range: {detail}")]
    OutOfRange { field: &'static str, detail: String },

    #[error("record exceeds bounded size of {limit} bytes")]
    OversizedRecord { limit: usize },
}

/// A record that failed parsing, retained for inspection rather than discarded.
#[derive(Debug, Clone)]
pub struct QuarantinedRecord {
    pub source: String,
    pub reason: String,
    /// The raw bytes, truncated to the bounded limit so a hostile feed cannot
    /// make us buffer unbounded data.
    pub raw_truncated: Vec<u8>,
}

impl QuarantinedRecord {
    pub fn new(source: &str, reason: String, raw: &[u8], limit: usize) -> Self {
        let n = raw.len().min(limit);
        Self {
            source: source.to_string(),
            reason,
            raw_truncated: raw[..n].to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarantine_truncates_to_bound() {
        let raw = vec![7u8; 1000];
        let q = QuarantinedRecord::new("s", "too big".into(), &raw, 128);
        assert_eq!(q.raw_truncated.len(), 128);
        assert_eq!(q.source, "s");
    }

    #[test]
    fn error_messages_are_descriptive() {
        let e = IngestError::OutOfRange { field: "lat", detail: "91".into() };
        assert!(e.to_string().contains("lat"));
    }
}
