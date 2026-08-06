//! Aeroscope server/demo binary.
//!
//! Subcommands wire the pipeline to a source:
//!   demo    run a short deterministic synthetic load and print a summary
//!   doctor  report host capabilities relevant to the full profiles
//!
//! The network transport and Cesium frontend connect to this pipeline; this
//! binary is the runnable backend slice and the basis for the make targets.

use aero_fusion::SourceTrust;
use aero_loadgen::{LoadGenConfig, LoadGenerator};
use aero_server::Pipeline;
use aero_spatial::Viewport;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("demo");
    match cmd {
        "demo" => run_demo(),
        "doctor" => run_doctor(),
        other => {
            eprintln!("unknown command '{other}'. try: demo | doctor");
            std::process::exit(2);
        }
    }
}

/// Run a short synthetic load through the full pipeline and print a summary.
/// This is the end-to-end demonstration referenced by `make demo`.
fn run_demo() {
    let tmp = env::temp_dir().join("aeroscope-demo.aero");
    let _ = std::fs::remove_file(&tmp);
    let trust = SourceTrust::default().with("synthetic", 5);
    let mut pipe =
        Pipeline::open(tmp.to_str().unwrap(), trust, /*is_replay=*/ true).expect("open pipeline");

    let mut gen = LoadGenerator::new(LoadGenConfig {
        aircraft_count: 2_000,
        seed: 42,
        duplicate_prob: 0.02,
        reorder_prob: 0.01,
        outage_prob: 0.0,
        source_id: "synthetic".into(),
    });

    // Subscribe to a viewport over the North Atlantic to exercise deltas.
    let vp = Viewport::new(20.0, 70.0, -80.0, 10.0);

    let start = std::time::Instant::now();
    let base_ms = 1_730_000_000_000i64;
    let ticks = 60; // 60 synthetic seconds at 1 Hz
    let mut sub_id = None;
    let mut deltas = 0u64;
    let mut coalesced = 0u64;

    for t in 0..ticks {
        let now_ms = base_ms + t * 1000;
        let obs = gen.tick(now_ms, t as f64);
        for o in &obs {
            pipe.ingest(o);
        }
        if t == 0 {
            let (id, _snap) = pipe.subscribe(vp, now_ms);
            sub_id = Some(id);
        } else if let Some(id) = sub_id {
            if let Some(msg) = pipe.flush_subscription(id, now_ms) {
                deltas += 1;
                if let aero_stream::ServerMessage::Delta(d) = msg {
                    if d.coalesced {
                        coalesced += 1;
                    }
                }
            }
        }
    }
    let elapsed = start.elapsed();
    let live = pipe.live_track_count(base_ms + ticks * 1000);

    println!("Aeroscope demo (synthetic replay, labeled synthetic; not a live feed)");
    println!("  ticks run            : {ticks}");
    println!("  wall time            : {elapsed:.2?}");
    println!("  observations accepted: {}", pipe.metrics.accepted);
    println!("  logged (durable)     : {}", pipe.metrics.logged);
    println!("  log duplicates       : {}", pipe.metrics.log_duplicates);
    println!("  fusion updated       : {}", pipe.metrics.fusion_updated);
    println!("  fusion late revisions: {}", pipe.metrics.fusion_late);
    println!("  fusion rejected      : {}", pipe.metrics.fusion_rejected);
    println!("  tracks indexed       : {}", pipe.metrics.tracks_indexed);
    println!("  live tracks          : {live}");
    println!("  deltas to subscriber : {deltas}");
    println!("  coalesced deltas     : {coalesced}");
    println!("  generator emitted    : {}", gen.emitted);
    println!("  generator duplicates : {}", gen.duplicates);
    println!("  generator reorders   : {}", gen.reordered);

    // A basic consistency check the demo asserts out loud. The log must catch
    // at least every injected exact duplicate; it also catches additional
    // dedup-key collisions from reordered records that share an event time, so
    // the relationship is >=, not ==.
    assert!(
        pipe.metrics.log_duplicates >= gen.duplicates,
        "the log must idempotently catch at least every injected duplicate"
    );
    // Durability check: accepted == logged + duplicates (nothing vanishes).
    assert_eq!(
        pipe.metrics.accepted,
        pipe.metrics.logged + pipe.metrics.log_duplicates,
        "every accepted observation is either logged or a caught duplicate"
    );
    println!("  OK: accepted == logged + duplicates (no silent loss)");
    println!(
        "  OK: log caught all {} injected duplicates (+ reorder collisions)",
        gen.duplicates
    );
    let _ = std::fs::remove_file(&tmp);
}

/// Report host capabilities relevant to the declared profiles.
fn run_doctor() {
    println!("aeroscope doctor");
    println!("  arch           : {}", env::consts::ARCH);
    println!("  os             : {}", env::consts::OS);
    println!("  pointer width  : {} bits", usize::BITS);
    println!(
        "  parallelism    : {}",
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(0)
    );
    // The full 50k/10k-ups/200-sub profile needs substantial resources; we only
    // report, never silently substitute a smaller workload.
    println!("note: the full acceptance profile (50,000 tracks, 10,000 updates/s,");
    println!("      200 subscribers, 6 hours) requires a declared reference host.");
    println!("      This diagnostic does not certify that profile.");
}
