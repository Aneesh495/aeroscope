//! Provider capability registry (A03).
//!
//! Every source declares what it can and cannot do: geographic limits, update
//! cadence, quotas, retention, which identifiers it carries, and which
//! enrichments it provides. Unsupported features stay visible but disabled, so
//! the UI can show "this source does not provide routes" rather than silently
//! omitting the capability.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FeatureSupport {
    Supported,
    /// Known to exist but switched off for this deployment (for example a
    /// licensed feature without an agreement). Visible, not hidden.
    DisabledUnlicensed,
    Unsupported,
}

/// Geographic limit of a source. `Global` does not mean global coverage; it
/// means the source does not restrict queries by region. Actual observed
/// coverage is measured separately (A23).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GeographicLimit {
    Global,
    /// Bounding box in degrees: (min_lat, min_lon, max_lat, max_lon).
    BoundingBox(f64, f64, f64, f64),
    /// A single receiver's reception range, roughly a radius in km.
    ReceiverRangeKm(f64),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderCapability {
    pub source_id: String,
    pub geographic_limit: GeographicLimit,
    /// Nominal update cadence in milliseconds.
    pub cadence_ms: u64,
    /// Requests allowed per time window, if the provider imposes a quota.
    pub quota_per_window: Option<u64>,
    pub quota_window_secs: Option<u64>,
    /// How long history may be retained per the provider's terms, in days.
    /// None means no provider-imposed retention limit (owned receiver).
    pub retention_days: Option<u32>,
    pub carries_callsign: bool,
    pub carries_baro_alt: bool,
    pub carries_geo_alt: bool,
    pub route_enrichment: FeatureSupport,
    /// Whether redistribution of raw records is permitted by the terms.
    pub redistribution: FeatureSupport,
}

#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    providers: Vec<ProviderCapability>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, cap: ProviderCapability) {
        // Replace an existing entry with the same id rather than duplicating.
        self.providers.retain(|p| p.source_id != cap.source_id);
        self.providers.push(cap);
    }
    pub fn get(&self, source_id: &str) -> Option<&ProviderCapability> {
        self.providers.iter().find(|p| p.source_id == source_id)
    }
    pub fn all(&self) -> &[ProviderCapability] {
        &self.providers
    }

    /// A readsb owned receiver: no quota, no provider retention limit, full
    /// altitude fields, no route enrichment, redistribution is the operator's
    /// own call so it is supported.
    pub fn readsb_default(source_id: &str, range_km: f64) -> ProviderCapability {
        ProviderCapability {
            source_id: source_id.to_string(),
            geographic_limit: GeographicLimit::ReceiverRangeKm(range_km),
            cadence_ms: 1000,
            quota_per_window: None,
            quota_window_secs: None,
            retention_days: None,
            carries_callsign: true,
            carries_baro_alt: true,
            carries_geo_alt: true,
            route_enrichment: FeatureSupport::Unsupported,
            redistribution: FeatureSupport::Supported,
        }
    }

    /// OpenSky REST: global query scope, quota-limited, redistribution requires
    /// an agreement so it is marked disabled-unlicensed by default.
    pub fn opensky_default(source_id: &str) -> ProviderCapability {
        ProviderCapability {
            source_id: source_id.to_string(),
            geographic_limit: GeographicLimit::Global,
            cadence_ms: 5000,
            quota_per_window: Some(4000),
            quota_window_secs: Some(86_400),
            retention_days: Some(0), // no local retention without an agreement
            carries_callsign: true,
            carries_baro_alt: true,
            carries_geo_alt: true,
            route_enrichment: FeatureSupport::Unsupported,
            redistribution: FeatureSupport::DisabledUnlicensed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_replaces_same_id() {
        let mut reg = CapabilityRegistry::new();
        reg.register(CapabilityRegistry::readsb_default("rx", 300.0));
        reg.register(CapabilityRegistry::readsb_default("rx", 400.0));
        assert_eq!(reg.all().len(), 1);
        match reg.get("rx").unwrap().geographic_limit {
            GeographicLimit::ReceiverRangeKm(r) => assert_eq!(r, 400.0),
            _ => panic!("wrong limit"),
        }
    }

    #[test]
    fn opensky_redistribution_disabled_by_default() {
        let cap = CapabilityRegistry::opensky_default("opensky");
        assert_eq!(cap.redistribution, FeatureSupport::DisabledUnlicensed);
        assert_eq!(cap.quota_per_window, Some(4000));
    }

    #[test]
    fn readsb_has_no_quota_and_owns_retention() {
        let cap = CapabilityRegistry::readsb_default("rx", 250.0);
        assert!(cap.quota_per_window.is_none());
        assert!(cap.retention_days.is_none());
    }
}
