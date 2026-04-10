//! Aviation identities, modeled as distinct entities.
//!
//! The brief insists these are different things: an ICAO24 address can be
//! erroneous or reused, callsigns change through a day, one airframe flies many
//! legs, and a schedule can be cancelled without ever producing an observation.
//! Keeping them in separate types stops the classic bug of treating a callsign
//! as a stable primary key.

use serde::{Deserialize, Serialize};

/// 24-bit ICAO Mode S address, rendered as 6 lowercase hex characters.
/// This identifies a transponder, which is usually but not always one airframe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Icao24(pub [u8; 3]);

impl Icao24 {
    /// Parse 6 hex chars. Returns None on bad input rather than guessing.
    pub fn parse_hex(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.len() != 6 {
            return None;
        }
        let mut out = [0u8; 3];
        for i in 0..3 {
            out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(Icao24(out))
    }

    pub fn to_hex(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.0[0], self.0[1], self.0[2])
    }

    /// The all-zero address is reserved/invalid and often a decode artifact.
    pub fn is_reserved(self) -> bool {
        self.0 == [0, 0, 0]
    }
}

/// A callsign as broadcast. Not unique across a day and not a stable key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Callsign(pub String);

impl Callsign {
    pub fn normalize(raw: &str) -> Option<Self> {
        let t = raw.trim().to_ascii_uppercase();
        if t.is_empty() {
            None
        } else {
            Some(Callsign(t))
        }
    }
}

/// Confidence in an enrichment or inference. The brief requires distinguishing
/// authoritative, inferred, stale, and absent facts; this makes that explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provenance {
    /// From an authoritative dataset at a pinned version.
    Authoritative,
    /// Derived by inference (for example a route guess). Treat with suspicion.
    Inferred,
    /// Was authoritative once but is older than the freshness policy allows.
    Stale,
    /// Not known. Explicitly absent, not defaulted.
    Absent,
}

/// An enrichment value carrying its provenance so the UI can label it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Enriched<T> {
    pub value: Option<T>,
    pub provenance: Provenance,
}

impl<T> Enriched<T> {
    pub fn authoritative(v: T) -> Self {
        Self { value: Some(v), provenance: Provenance::Authoritative }
    }
    pub fn inferred(v: T) -> Self {
        Self { value: Some(v), provenance: Provenance::Inferred }
    }
    pub fn absent() -> Self {
        Self { value: None, provenance: Provenance::Absent }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icao24_roundtrip_and_reserved() {
        let a = Icao24::parse_hex("4ca7b3").unwrap();
        assert_eq!(a.to_hex(), "4ca7b3");
        assert!(!a.is_reserved());
        assert!(Icao24::parse_hex("000000").unwrap().is_reserved());
    }

    #[test]
    fn icao24_rejects_bad_input() {
        assert!(Icao24::parse_hex("zzzz").is_none());
        assert!(Icao24::parse_hex("4ca7b").is_none()); // too short
        assert!(Icao24::parse_hex("4ca7b3ff").is_none()); // too long
    }

    #[test]
    fn callsign_normalizes_and_rejects_empty() {
        assert_eq!(Callsign::normalize("  dlh42 ").unwrap().0, "DLH42");
        assert!(Callsign::normalize("   ").is_none());
    }

    #[test]
    fn enrichment_carries_provenance() {
        let e = Enriched::inferred("LAX");
        assert_eq!(e.provenance, Provenance::Inferred);
        let a: Enriched<String> = Enriched::absent();
        assert_eq!(a.provenance, Provenance::Absent);
        assert!(a.value.is_none());
    }
}
