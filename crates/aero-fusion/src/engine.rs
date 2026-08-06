//! Event-time fusion engine (A05).
//!
//! This is a deterministic, event-time state machine. Observations are fed in
//! and may arrive out of order; a watermark tracks how far event time has
//! advanced, and late observations older than the watermark minus the allowed
//! lateness are recorded as late and do not revise already-delivered state
//! (the client would instead receive an explicit correction event, modeled
//! here as a `FusionOutcome::LateRevision`).
//!
//! Conflict handling (A05): when two sources report incompatible positions for
//! the same aircraft at close event times, we do NOT average them. We keep both
//! as evidence and select one by a declared rule (source trust, then position
//! quality, then recency), recording why. Altitudes and timestamps whose
//! meanings differ are never averaged.

use crate::lifecycle::{LifecycleConfig, TrackState};
use crate::plausibility::{self, Plausibility};
use aero_geo::{haversine_m, Geodetic};
use aero_types::{Icao24, Observation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Declared source trust, higher wins a conflict. Owned receivers are trusted
/// above remote aggregators by default; configurable per deployment.
#[derive(Debug, Clone, Default)]
pub struct SourceTrust(HashMap<String, u32>);

impl SourceTrust {
    pub fn with(mut self, source: &str, trust: u32) -> Self {
        self.0.insert(source.to_string(), trust);
        self
    }
    fn trust_of(&self, source: &str) -> u32 {
        *self.0.get(source).unwrap_or(&0)
    }
}

/// A piece of retained evidence contributing to a fused state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub source: String,
    pub event_time_ms: i64,
    pub position: Option<Geodetic>,
    pub plausibility: Plausibility,
}

/// The fused state of one aircraft at a point in processing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FusedState {
    pub icao24_hex: String,
    pub position: Option<Geodetic>,
    pub event_time_ms: i64,
    pub state: TrackState,
    /// The source that was selected for the current position and why.
    pub selected_source: Option<String>,
    pub selection_reason: SelectionReason,
    /// All evidence considered at the latest event time, retained not averaged.
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionReason {
    SingleSource,
    HigherTrust,
    BetterRecency,
    None,
}

/// What happened when an observation was fed in.
#[derive(Debug, Clone, PartialEq)]
pub enum FusionOutcome {
    /// Accepted and advanced the track's state.
    Updated,
    /// Older than the watermark tolerance: recorded as a late revision.
    LateRevision,
    /// Rejected by a plausibility check; evidence retained, state unchanged.
    Rejected(Plausibility),
}

#[derive(Debug, Clone)]
struct TrackInternal {
    position: Option<Geodetic>,
    event_time_ms: i64,
    selected_source: Option<String>,
    selection_reason: SelectionReason,
    evidence: Vec<Evidence>,
}

pub struct FusionEngine {
    trust: SourceTrust,
    lifecycle: LifecycleConfig,
    /// Allowed lateness: observations within this many ms behind the watermark
    /// are still fused normally; older ones are late revisions.
    allowed_lateness_ms: i64,
    watermark_ms: i64,
    tracks: HashMap<Icao24, TrackInternal>,
    /// Counters for G03 exact independent counts.
    pub count_updated: u64,
    pub count_late: u64,
    pub count_rejected: u64,
}

impl FusionEngine {
    pub fn new(trust: SourceTrust, lifecycle: LifecycleConfig, allowed_lateness_ms: i64) -> Self {
        Self {
            trust,
            lifecycle,
            allowed_lateness_ms,
            watermark_ms: i64::MIN,
            tracks: HashMap::new(),
            count_updated: 0,
            count_late: 0,
            count_rejected: 0,
        }
    }

    pub fn watermark_ms(&self) -> i64 {
        self.watermark_ms
    }

    /// Feed one observation. Deterministic given the same input order.
    pub fn ingest(&mut self, obs: &Observation) -> FusionOutcome {
        let (event_ms, _) = obs.effective_event_time_ms();

        let prev = self.tracks.get(&obs.icao24);
        let prev_pos = prev.and_then(|t| t.position);
        let prev_ms = prev.map(|t| t.event_time_ms);

        // Late-event policy runs first. An out-of-order event older than the
        // watermark minus allowed lateness is an expected event-time situation,
        // not a plausibility failure, so we classify it as a late revision
        // rather than flagging spurious time reversal against newer live state.
        // History may be revised; the live watermark never moves backwards.
        // Saturating subtraction keeps the unset watermark (i64::MIN) safe.
        let late_threshold = self.watermark_ms.saturating_sub(self.allowed_lateness_ms);
        if self.watermark_ms != i64::MIN && event_ms < late_threshold {
            self.count_late += 1;
            let evidence = Evidence {
                source: obs.source.0.clone(),
                event_time_ms: event_ms,
                position: obs.position,
                plausibility: Plausibility::Ok,
            };
            if let Some(t) = self.tracks.get_mut(&obs.icao24) {
                t.evidence.push(evidence);
            }
            return FusionOutcome::LateRevision;
        }

        let plaus = plausibility::check(prev_pos, prev_ms, obs);
        let evidence = Evidence {
            source: obs.source.0.clone(),
            event_time_ms: event_ms,
            position: obs.position,
            plausibility: plaus,
        };

        if plaus != Plausibility::Ok {
            // Retain the conflicting evidence on the track, leave state as is.
            self.tracks
                .entry(obs.icao24)
                .or_insert_with(|| TrackInternal {
                    position: prev_pos,
                    event_time_ms: prev_ms.unwrap_or(event_ms),
                    selected_source: None,
                    selection_reason: SelectionReason::None,
                    evidence: Vec::new(),
                })
                .evidence
                .push(evidence);
            self.count_rejected += 1;
            return FusionOutcome::Rejected(plaus);
        }

        // Advance the watermark monotonically.
        if event_ms > self.watermark_ms {
            self.watermark_ms = event_ms;
        }

        // Conflict selection against the current track at a close event time.
        let (selected_source, reason) = self.select(prev, &evidence, event_ms);

        let entry = self
            .tracks
            .entry(obs.icao24)
            .or_insert_with(|| TrackInternal {
                position: None,
                event_time_ms: event_ms,
                selected_source: None,
                selection_reason: SelectionReason::None,
                evidence: Vec::new(),
            });

        // Prune evidence older than this event time so the retained set reflects
        // the current decision point (bounded memory, A11).
        entry
            .evidence
            .retain(|e| e.event_time_ms >= event_ms.saturating_sub(2_000));
        entry.evidence.push(evidence);

        // Only move the position if this source was selected.
        if selected_source.as_deref() == Some(obs.source.0.as_str()) {
            entry.position = obs.position;
            entry.event_time_ms = event_ms;
            entry.selected_source = selected_source;
            entry.selection_reason = reason;
        } else if entry.selected_source.is_none() {
            entry.position = obs.position;
            entry.event_time_ms = event_ms;
            entry.selected_source = Some(obs.source.0.clone());
            entry.selection_reason = SelectionReason::SingleSource;
        }

        self.count_updated += 1;
        FusionOutcome::Updated
    }

    /// Decide which source provides the position. Declared rule order:
    /// higher trust, then more recent, otherwise keep the single source.
    fn select(
        &self,
        prev: Option<&TrackInternal>,
        new_ev: &Evidence,
        event_ms: i64,
    ) -> (Option<String>, SelectionReason) {
        let prev = match prev {
            Some(p) if p.selected_source.is_some() => p,
            _ => return (Some(new_ev.source.clone()), SelectionReason::SingleSource),
        };
        let prev_src = prev.selected_source.clone().unwrap();
        if prev_src == new_ev.source {
            return (Some(new_ev.source.clone()), SelectionReason::BetterRecency);
        }
        let prev_trust = self.trust.trust_of(&prev_src);
        let new_trust = self.trust.trust_of(&new_ev.source);
        if new_trust > prev_trust {
            (Some(new_ev.source.clone()), SelectionReason::HigherTrust)
        } else if new_trust < prev_trust {
            (Some(prev_src), SelectionReason::HigherTrust)
        } else if event_ms >= prev.event_time_ms {
            (Some(new_ev.source.clone()), SelectionReason::BetterRecency)
        } else {
            (Some(prev_src), SelectionReason::BetterRecency)
        }
    }

    /// Snapshot a track's fused state as of `now_ms`, classifying lifecycle.
    pub fn snapshot(&self, icao24: &Icao24, now_ms: i64) -> Option<FusedState> {
        let t = self.tracks.get(icao24)?;
        let age = now_ms - t.event_time_ms;
        Some(FusedState {
            icao24_hex: icao24.to_hex(),
            position: t.position,
            event_time_ms: t.event_time_ms,
            state: self.lifecycle.classify(age),
            selected_source: t.selected_source.clone(),
            selection_reason: t.selection_reason,
            evidence: t.evidence.clone(),
        })
    }

    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    /// For G02 dateline tests: how far two retained positions are apart.
    pub fn evidence_spread_m(&self, icao24: &Icao24) -> Option<f64> {
        let t = self.tracks.get(icao24)?;
        let positions: Vec<Geodetic> = t.evidence.iter().filter_map(|e| e.position).collect();
        if positions.len() < 2 {
            return Some(0.0);
        }
        let mut max = 0.0;
        for i in 0..positions.len() {
            for j in i + 1..positions.len() {
                max = f64::max(max, haversine_m(&positions[i], &positions[j]));
            }
        }
        Some(max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_types::*;

    fn obs(src: &str, lat: f64, lon: f64, t: i64) -> Observation {
        Observation {
            source: SourceId(src.into()),
            icao24: Icao24::parse_hex("4ca7b3").unwrap(),
            callsign: None,
            observation_time: MaybeTime::Known(ObservationTime(t)),
            publish_time: MaybeTime::Absent,
            ingest_wall_time: IngestWallTime(t),
            position: Some(Geodetic::new(lat, lon, 10000.0)),
            baro_alt: Some(BaroAltM(10000.0)),
            geo_alt: Some(GeoAltM(10000.0)),
            ground_speed: Some(SpeedMps(230.0)),
            true_track: Some(TrueTrackDeg(90.0)),
            vertical_rate: Some(VerticalRateMps(0.0)),
            ground_state: GroundState::Airborne,
            source_sequence: None,
        }
    }

    fn engine() -> FusionEngine {
        let trust = SourceTrust::default().with("rx", 10).with("remote", 5);
        FusionEngine::new(trust, LifecycleConfig::default(), 2_000)
    }

    #[test]
    fn single_source_updates_and_advances_watermark() {
        let mut e = engine();
        assert_eq!(
            e.ingest(&obs("rx", 51.0, 0.0, 1_000)),
            FusionOutcome::Updated
        );
        assert_eq!(
            e.ingest(&obs("rx", 51.0, 0.03, 11_000)),
            FusionOutcome::Updated
        );
        assert_eq!(e.watermark_ms(), 11_000);
        assert_eq!(e.count_updated, 2);
    }

    #[test]
    fn higher_trust_source_wins_conflict_without_averaging() {
        let mut e = engine();
        // Remote reports one position; owned receiver reports a different one
        // at the same event time. Owned (higher trust) should be selected and
        // the position must be exactly the owned one, not an average.
        e.ingest(&obs("remote", 10.0, 10.0, 1_000));
        e.ingest(&obs("rx", 20.0, 20.0, 1_000));
        let snap = e
            .snapshot(&Icao24::parse_hex("4ca7b3").unwrap(), 1_000)
            .unwrap();
        assert_eq!(snap.selected_source.as_deref(), Some("rx"));
        assert_eq!(snap.selection_reason, SelectionReason::HigherTrust);
        let p = snap.position.unwrap();
        assert!((p.lat_deg - 20.0).abs() < 1e-9); // exactly owned, not 15.0
                                                  // Both evidences retained.
        assert_eq!(snap.evidence.len(), 2);
    }

    #[test]
    fn impossible_jump_rejected_but_evidence_retained() {
        let mut e = engine();
        e.ingest(&obs("rx", 51.0, 0.0, 1_000));
        let out = e.ingest(&obs("rx", -30.0, 150.0, 2_000));
        assert_eq!(out, FusionOutcome::Rejected(Plausibility::ImpossibleJump));
        assert_eq!(e.count_rejected, 1);
        let snap = e
            .snapshot(&Icao24::parse_hex("4ca7b3").unwrap(), 2_000)
            .unwrap();
        // Position unchanged from the plausible first observation.
        assert!((snap.position.unwrap().lon_deg - 0.0).abs() < 1e-9);
        // The rejected evidence is still retained for inspection.
        assert!(snap
            .evidence
            .iter()
            .any(|ev| ev.plausibility == Plausibility::ImpossibleJump));
    }

    #[test]
    fn late_event_recorded_not_applied_to_watermark() {
        let mut e = engine();
        e.ingest(&obs("rx", 51.0, 0.0, 10_000));
        // Far behind watermark (10_000) minus lateness (2_000) -> late.
        let out = e.ingest(&obs("rx", 51.0, 0.0, 1_000));
        assert_eq!(out, FusionOutcome::LateRevision);
        assert_eq!(e.watermark_ms(), 10_000); // not moved backward
        assert_eq!(e.count_late, 1);
    }

    #[test]
    fn lifecycle_reflected_in_snapshot() {
        let mut e = engine();
        e.ingest(&obs("rx", 51.0, 0.0, 1_000));
        let id = Icao24::parse_hex("4ca7b3").unwrap();
        assert_eq!(e.snapshot(&id, 1_000).unwrap().state, TrackState::Observed);
        assert_eq!(e.snapshot(&id, 70_000).unwrap().state, TrackState::Lost);
    }
}
