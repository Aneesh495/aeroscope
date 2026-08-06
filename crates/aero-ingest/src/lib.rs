//! Source adapters for Aeroscope (A01, A02, A03).
//!
//! Three explicit source modes are supported:
//! - an owned/local receiver via the readsb JSON adapter;
//! - a licensed remote feed via the OpenSky OAuth2 adapter (disabled unless an
//!   agreement is acknowledged);
//! - clearly labeled recorded replay, handled downstream in the server.
//!
//! Every adapter normalizes into `aero_types::Observation`, preserves explicit
//! nulls, converts units at the edge, attaches source attribution, and reports
//! malformed records for quarantine instead of dropping them silently.

pub mod capability;
pub mod opensky;
pub mod readsb;

pub use capability::{CapabilityRegistry, FeatureSupport, GeographicLimit, ProviderCapability};
pub use opensky::{parse_states_all, CachedToken, HttpTransport, OpenSkyClient, OperationalGuard};
pub use readsb::{parse_readsb, ReadsbParse, MAX_DOC_BYTES};
