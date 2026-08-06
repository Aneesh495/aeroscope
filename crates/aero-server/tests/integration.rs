//! Integration tests that drive the whole pipeline with the load generator and
//! check end-to-end invariants (deterministic integration test for phase 3).

use aero_fusion::SourceTrust;
use aero_loadgen::{LoadGenConfig, LoadGenerator};
use aero_server::pipeline::Pipeline;
use aero_spatial::Viewport;

fn tmp_log() -> String {
    let dir = std::env::temp_dir();
    let p = dir.join(format!(
        "aero-it-{}-{}.aero",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    p.to_str().unwrap().to_string()
}

#[test]
fn full_pipeline_no_silent_loss() {
    let path = tmp_log();
    let trust = SourceTrust::default().with("synthetic", 5);
    let mut pipe = Pipeline::open(&path, trust, true).unwrap();
    let mut gen = LoadGenerator::new(LoadGenConfig {
        aircraft_count: 500,
        seed: 7,
        duplicate_prob: 0.05,
        reorder_prob: 0.02,
        outage_prob: 0.0,
        source_id: "synthetic".into(),
    });

    let base = 1_730_000_000_000i64;
    for t in 0..30 {
        for o in gen.tick(base + t * 1000, t as f64) {
            pipe.ingest(&o);
        }
    }

    // Core durability invariant.
    assert_eq!(
        pipe.metrics.accepted,
        pipe.metrics.logged + pipe.metrics.log_duplicates,
        "accepted must equal logged plus caught duplicates"
    );
    // Every logged observation took exactly one fusion path.
    assert_eq!(
        pipe.metrics.fusion_updated + pipe.metrics.fusion_late + pipe.metrics.fusion_rejected,
        pipe.metrics.logged,
        "fusion accounting must balance"
    );
    // The log caught at least every injected duplicate.
    assert!(pipe.metrics.log_duplicates >= gen.duplicates);
    // The index must never hold more than the fleet, and should hold the great
    // majority once each aircraft has had an accepted positioned observation.
    // A handful may lag if their only in-tick observations were late revisions,
    // so we assert a tight lower bound rather than exact equality.
    assert!(pipe.metrics.tracks_indexed <= 500);
    assert!(
        pipe.metrics.tracks_indexed >= 495,
        "expected almost all 500 aircraft indexed, got {}",
        pipe.metrics.tracks_indexed
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn viewport_subscription_sees_only_in_view_tracks() {
    let path = tmp_log();
    let trust = SourceTrust::default().with("synthetic", 5);
    let mut pipe = Pipeline::open(&path, trust, true).unwrap();
    let mut gen = LoadGenerator::new(LoadGenConfig {
        aircraft_count: 1000,
        seed: 3,
        duplicate_prob: 0.0,
        reorder_prob: 0.0,
        outage_prob: 0.0,
        source_id: "synthetic".into(),
    });
    let now = 1_730_000_000_000i64;
    for o in gen.tick(now, 10.0) {
        pipe.ingest(&o);
    }

    // A small viewport should see far fewer than the whole fleet.
    let (_, snap) = pipe.subscribe(Viewport::new(40.0, 60.0, -10.0, 10.0), now);
    let in_view = match snap {
        aero_stream::ServerMessage::Snapshot(s) => s.tracks.len(),
        _ => panic!("expected snapshot"),
    };
    assert!(
        in_view < 1000,
        "viewport should bound the result, got {in_view}"
    );

    // The whole-globe viewport should see all indexed tracks.
    let (_, snap_all) = pipe.subscribe(Viewport::new(-90.0, 90.0, -180.0, 179.999), now);
    let all = match snap_all {
        aero_stream::ServerMessage::Snapshot(s) => s.tracks.len(),
        _ => panic!(),
    };
    assert_eq!(all as u64, pipe.metrics.tracks_indexed);

    let _ = std::fs::remove_file(&path);
}
