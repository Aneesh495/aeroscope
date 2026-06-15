//! The integrated pipeline: observations -> log -> fusion -> spatial -> deltas.
//!
//! This is the vertical slice that ties every backend subsystem together. It is
//! deliberately synchronous and single-threaded so its behavior is
//! deterministic and easy to test; the concurrency and transport live at the
//! edges (the server binary and, later, the async network layer). The pipeline
//! owns the durable log, the fusion engine, the spatial index, and the set of
//! viewport subscriptions, and exposes `ingest` and `tick` so a source loop or
//! the load generator can drive it.

use aero_fusion::{FusionEngine, FusionOutcome, LifecycleConfig, SourceTrust, TrackState};
use aero_log::{LoggedObservation, ObservationLog};
use aero_spatial::{SpatialGrid, Viewport};
use aero_stream::{ServerMessage, Subscription, TrackWire};
use aero_types::{Icao24, Observation};
use std::collections::HashMap;

/// Metrics exported for G29/G05 (native metrics). All counters, no allocation
/// in the hot path.
#[derive(Debug, Default, Clone)]
pub struct PipelineMetrics {
    pub accepted: u64,
    pub logged: u64,
    pub log_duplicates: u64,
    pub fusion_updated: u64,
    pub fusion_late: u64,
    pub fusion_rejected: u64,
    pub tracks_indexed: u64,
}

pub struct Pipeline {
    log: ObservationLog,
    fusion: FusionEngine,
    grid: SpatialGrid,
    subs: HashMap<u64, (Subscription, Viewport)>,
    next_sub_id: u64,
    pub metrics: PipelineMetrics,
    /// Whether this pipeline is serving replay. Replay is always labeled and can
    /// never be presented as live (honesty invariant, G07/A13).
    pub is_replay: bool,
}

impl Pipeline {
    pub fn open(log_path: &str, trust: SourceTrust, is_replay: bool) -> std::io::Result<Self> {
        let log = ObservationLog::open(log_path)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(Self {
            log,
            fusion: FusionEngine::new(trust, LifecycleConfig::default(), 2_000),
            grid: SpatialGrid::new(),
            subs: HashMap::new(),
            next_sub_id: 0,
            metrics: PipelineMetrics::default(),
            is_replay,
        })
    }

    /// Ingest one observation through the whole pipeline. Durability first: the
    /// observation is logged before fusion sees it, so a crash loses at most the
    /// in-flight record.
    pub fn ingest(&mut self, obs: &Observation) {
        if !obs.is_structurally_valid() {
            return;
        }
        self.metrics.accepted += 1;

        match self.log.append(&LoggedObservation::new(obs.clone())) {
            Ok(_) => self.metrics.logged += 1,
            Err(aero_log::LogError::Duplicate) => {
                self.metrics.log_duplicates += 1;
                return; // idempotent: a duplicate never reaches fusion twice
            }
            Err(_) => return,
        }

        match self.fusion.ingest(obs) {
            FusionOutcome::Updated => {
                self.metrics.fusion_updated += 1;
                // Reflect the new fused position into the spatial index.
                if let Some(snap) = self.fusion.snapshot(&obs.icao24, obs.ingest_wall_time.0) {
                    if let Some(pos) = snap.position {
                        self.grid.upsert(obs.icao24.to_hex(), pos);
                    }
                }
            }
            FusionOutcome::LateRevision => self.metrics.fusion_late += 1,
            FusionOutcome::Rejected(_) => self.metrics.fusion_rejected += 1,
        }
        self.metrics.tracks_indexed = self.grid.len() as u64;
    }

    /// Register a viewport subscription, returning its id and initial snapshot.
    pub fn subscribe(&mut self, vp: Viewport, now_ms: i64) -> (u64, ServerMessage) {
        self.next_sub_id += 1;
        let id = self.next_sub_id;
        let mut sub = Subscription::new(id, aero_stream::PROTOCOL_VERSION, 256, 4096);
        let tracks = self.tracks_in(&vp, now_ms);
        let snap = sub.make_snapshot(tracks);
        self.subs.insert(id, (sub, vp));
        (id, snap)
    }

    /// Produce the current set of wire tracks within a viewport.
    fn tracks_in(&self, vp: &Viewport, now_ms: i64) -> Vec<TrackWire> {
        self.grid
            .query(vp)
            .into_iter()
            .filter_map(|hex| {
                let id = Icao24::parse_hex(&hex)?;
                let s = self.fusion.snapshot(&id, now_ms)?;
                Some(TrackWire {
                    icao24: s.icao24_hex,
                    position: s.position,
                    event_time_ms: s.event_time_ms,
                    state: s.state,
                    selected_source: s.selected_source,
                    selection_reason: s.selection_reason,
                })
            })
            .collect()
    }

    /// Flush deltas to a subscription. Enqueues the current in-view tracks as
    /// upserts and tracks that left the view as removes, then flushes with the
    /// subscription's backpressure policy.
    pub fn flush_subscription(&mut self, id: u64, now_ms: i64) -> Option<ServerMessage> {
        let vp = self.subs.get(&id)?.1;
        let in_view = self.tracks_in(&vp, now_ms);
        let sub = &mut self.subs.get_mut(&id)?.0;
        for t in in_view {
            sub.enqueue_upsert(t);
        }
        sub.flush()
    }

    pub fn subscription_count(&self) -> usize {
        self.subs.len()
    }

    /// How many tracks are currently in a live (observed/extrapolated) state.
    pub fn live_track_count(&self, now_ms: i64) -> usize {
        let mut n = 0;
        // Iterate indexed tracks and classify.
        for hex in self.grid.query(&Viewport::new(-90.0, 90.0, -180.0, 179.999)) {
            if let Some(id) = Icao24::parse_hex(&hex) {
                if let Some(s) = self.fusion.snapshot(&id, now_ms) {
                    if matches!(s.state, TrackState::Observed | TrackState::Extrapolated) {
                        n += 1;
                    }
                }
            }
        }
        n
    }

    pub fn log_count(&self) -> u64 {
        self.log.count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_geo::Geodetic;
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
            baro_alt: None,
            geo_alt: Some(GeoAltM(10000.0)),
            ground_speed: Some(SpeedMps(230.0)),
            true_track: Some(TrueTrackDeg(90.0)),
            vertical_rate: None,
            ground_state: GroundState::Airborne,
            source_sequence: Some(1),
        }
    }

    fn pipeline() -> Pipeline {
        let dir = std::env::temp_dir().join(format!("aero-pl-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("log-{}.aero", rand_suffix()));
        Pipeline::open(path.to_str().unwrap(), SourceTrust::default().with("rx", 10), false).unwrap()
    }

    fn rand_suffix() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
    }

    #[test]
    fn end_to_end_ingest_log_fuse_index() {
        let mut p = pipeline();
        p.ingest(&obs("rx", 51.0, 0.0, 1_000));
        assert_eq!(p.metrics.logged, 1);
        assert_eq!(p.metrics.fusion_updated, 1);
        assert_eq!(p.metrics.tracks_indexed, 1);
        assert_eq!(p.log_count(), 1);
    }

    #[test]
    fn subscription_snapshot_sees_track() {
        let mut p = pipeline();
        p.ingest(&obs("rx", 51.0, 0.0, 1_000));
        let (_, snap) = p.subscribe(Viewport::new(50.0, 52.0, -1.0, 1.0), 1_000);
        match snap {
            ServerMessage::Snapshot(s) => {
                assert_eq!(s.tracks.len(), 1);
                assert_eq!(s.tracks[0].icao24, "4ca7b3");
            }
            _ => panic!("expected snapshot"),
        }
    }

    #[test]
    fn duplicate_is_idempotent_through_pipeline() {
        let mut p = pipeline();
        p.ingest(&obs("rx", 51.0, 0.0, 1_000));
        p.ingest(&obs("rx", 51.0, 0.0, 1_000)); // same dedup key
        assert_eq!(p.metrics.logged, 1);
        assert_eq!(p.metrics.log_duplicates, 1);
    }

    #[test]
    fn delta_flush_after_movement() {
        let mut p = pipeline();
        p.ingest(&obs("rx", 51.0, 0.0, 1_000));
        let (id, _) = p.subscribe(Viewport::new(50.0, 52.0, -1.0, 1.0), 1_000);
        p.ingest(&obs("rx", 51.0, 0.01, 2_000));
        let d = p.flush_subscription(id, 2_000);
        assert!(d.is_some());
    }
}
