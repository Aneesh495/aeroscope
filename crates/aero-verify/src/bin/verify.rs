//! Independent verifier binary.
//!
//! Reads an evidence bundle and recomputes its decisive quantities from raw
//! records plus a bounded replay through the real engine. It never trusts a
//! saved pass boolean. Exits nonzero if any decisive check fails, if a required
//! file is missing, or if the bundle is internally inconsistent.

use aero_verify::{verify, Evidence};
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let evidence_dir = flag(&args, "--evidence").unwrap_or_else(|| {
        eprintln!("usage: aero-verify --evidence <dir>");
        std::process::exit(2);
    });
    let path = PathBuf::from(&evidence_dir).join("ACCEPTANCE.json");
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {}: {e}", path.display());
            std::process::exit(2);
        }
    };
    let e: Evidence = match serde_json::from_slice(&bytes) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("evidence is not valid: {err}");
            std::process::exit(2);
        }
    };

    println!("verifying evidence bundle: {}", path.display());
    println!(
        "  run_id={} profile={} seed={} fleet={} ticks={}",
        e.run_id, e.profile, e.seed, e.aircraft_count, e.ticks
    );

    let (ok, results) = verify(&e);
    for r in &results {
        println!(
            "  [{}] {}: {}",
            if r.ok { "PASS" } else { "FAIL" },
            r.name,
            r.reason
        );
    }

    // Report claimed gate statuses transparently (not trusted for decisive
    // numbers, but shown so a reader sees what was and was not certified).
    println!("  claimed gates:");
    for c in &e.claims {
        println!("    {} = {:?}  ({})", c.gate, c.status, c.detail);
    }

    if ok {
        println!("VERIFY: accepted (all decisive checks recomputed and held)");
    } else {
        println!("VERIFY: rejected (a decisive check failed for a semantic reason)");
        std::process::exit(1);
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}
