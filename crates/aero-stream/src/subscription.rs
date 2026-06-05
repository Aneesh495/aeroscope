//! Per-subscription state: sequencing, bounded queue, backpressure, and the
//! reconnect resync invariant (A10, A11).
//!
//! Each subscriber owns a `Subscription` with:
//! - a monotonic sequence counter so the client can detect gaps;
//! - the id of the snapshot its deltas build on;
//! - a bounded outbound queue. When the queue would overflow (a slow consumer),
//!   we coalesce pending per-track updates to the latest value. If even the
//!   coalesced backlog exceeds a hard cap, we drop the backlog and force a
//!   resync rather than block ingestion or grow memory without bound.
//!
//! Resync invariant: on `Resume`, if the client's last snapshot id does not
//! match the subscription's current snapshot id, the client is sent a fresh
//! snapshot. A delta is only ever sent for the matching base, so an old delta
//! can never be applied to a newer snapshot.

use crate::protocol::{
    Delta, ServerMessage, Snapshot, SnapshotId, Seq, TrackWire,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueuePolicy {
    /// Coalesce pending updates per track to the latest, then if still over the
    /// hard cap, force a resync.
    CoalesceThenResync,
}

pub struct Subscription {
    pub id: u64,
    pub version: u16,
    next_seq: Seq,
    current_snapshot_id: SnapshotId,
    /// Highest seq the client has acknowledged.
    acked_seq: Seq,
    /// Pending coalesced upserts keyed by icao24.
    pending_upserts: HashMap<String, TrackWire>,
    pending_removes: Vec<String>,
    /// Soft cap: beyond this we coalesce. Hard cap: beyond this we resync.
    soft_cap: usize,
    hard_cap: usize,
    coalesced_since_flush: bool,
    policy: QueuePolicy,
}

impl Subscription {
    pub fn new(id: u64, version: u16, soft_cap: usize, hard_cap: usize) -> Self {
        Self {
            id,
            version,
            next_seq: 0,
            current_snapshot_id: 0,
            acked_seq: 0,
            pending_upserts: HashMap::new(),
            pending_removes: Vec::new(),
            soft_cap,
            hard_cap,
            coalesced_since_flush: false,
            policy: QueuePolicy::CoalesceThenResync,
        }
    }

    fn take_seq(&mut self) -> Seq {
        self.next_seq += 1;
        self.next_seq
    }

    /// Produce the initial snapshot for a newly subscribed client.
    pub fn make_snapshot(&mut self, tracks: Vec<TrackWire>) -> ServerMessage {
        self.current_snapshot_id += 1;
        self.pending_upserts.clear();
        self.pending_removes.clear();
        self.coalesced_since_flush = false;
        let seq = self.take_seq();
        ServerMessage::Snapshot(Snapshot {
            snapshot_id: self.current_snapshot_id,
            seq,
            tracks,
        })
    }

    /// Enqueue an update. Coalescing means a second update for the same track
    /// replaces the first, so a slow consumer sees the latest state, not every
    /// intermediate one. Returns whether coalescing occurred.
    pub fn enqueue_upsert(&mut self, track: TrackWire) {
        if self.pending_upserts.contains_key(&track.icao24) {
            self.coalesced_since_flush = true;
        }
        self.pending_removes.retain(|r| r != &track.icao24);
        self.pending_upserts.insert(track.icao24.clone(), track);
    }

    pub fn enqueue_remove(&mut self, icao24: String) {
        self.pending_upserts.remove(&icao24);
        if !self.pending_removes.contains(&icao24) {
            self.pending_removes.push(icao24);
        }
    }

    fn pending_len(&self) -> usize {
        self.pending_upserts.len() + self.pending_removes.len()
    }

    /// Flush pending changes as a delta, or force a resync if the coalesced
    /// backlog still exceeds the hard cap (slow consumer that cannot keep up).
    pub fn flush(&mut self) -> Option<ServerMessage> {
        let _ = self.policy; // single policy today; field documents intent
        if self.pending_len() == 0 {
            return None;
        }
        if self.pending_len() > self.hard_cap {
            // Even after coalescing the client is too far behind. Drop backlog
            // and require a fresh snapshot. Never block ingestion.
            self.pending_upserts.clear();
            self.pending_removes.clear();
            return Some(ServerMessage::ResyncRequired {
                reason: format!(
                    "backlog exceeded hard cap {} after coalescing",
                    self.hard_cap
                ),
            });
        }
        let seq = self.take_seq();
        let coalesced = self.coalesced_since_flush || self.pending_len() > self.soft_cap;
        let upserts: Vec<TrackWire> = self.pending_upserts.drain().map(|(_, v)| v).collect();
        let removes = std::mem::take(&mut self.pending_removes);
        self.coalesced_since_flush = false;
        Some(ServerMessage::Delta(Delta {
            base_snapshot_id: self.current_snapshot_id,
            seq,
            upserts,
            removes,
            coalesced,
        }))
    }

    pub fn ack(&mut self, seq: Seq) {
        if seq > self.acked_seq {
            self.acked_seq = seq;
        }
    }

    pub fn acked_seq(&self) -> Seq {
        self.acked_seq
    }

    /// Handle a reconnect. The resync invariant: only matching snapshot ids may
    /// continue with deltas; otherwise the caller must send a fresh snapshot.
    pub fn can_resume_with_deltas(&self, last_snapshot_id: SnapshotId) -> bool {
        last_snapshot_id == self.current_snapshot_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_fusion::{SelectionReason, TrackState};

    fn track(id: &str, lon: f64) -> TrackWire {
        TrackWire {
            icao24: id.into(),
            position: Some(aero_geo::Geodetic::new(0.0, lon, 10000.0)),
            event_time_ms: 1000,
            state: TrackState::Observed,
            selected_source: Some("rx".into()),
            selection_reason: SelectionReason::SingleSource,
        }
    }

    #[test]
    fn sequence_numbers_are_monotonic() {
        let mut s = Subscription::new(1, 1, 100, 1000);
        let snap = s.make_snapshot(vec![]);
        let s1 = match snap { ServerMessage::Snapshot(sn) => sn.seq, _ => panic!() };
        s.enqueue_upsert(track("a", 0.0));
        let d = s.flush().unwrap();
        let s2 = match d { ServerMessage::Delta(d) => d.seq, _ => panic!() };
        assert!(s2 > s1);
    }

    #[test]
    fn deltas_reference_current_snapshot() {
        let mut s = Subscription::new(1, 1, 100, 1000);
        s.make_snapshot(vec![]);
        s.enqueue_upsert(track("a", 0.0));
        match s.flush().unwrap() {
            ServerMessage::Delta(d) => assert_eq!(d.base_snapshot_id, 1),
            _ => panic!(),
        }
    }

    #[test]
    fn coalescing_keeps_only_latest_per_track() {
        let mut s = Subscription::new(1, 1, 100, 1000);
        s.make_snapshot(vec![]);
        s.enqueue_upsert(track("a", 1.0));
        s.enqueue_upsert(track("a", 2.0)); // coalesces with the first
        match s.flush().unwrap() {
            ServerMessage::Delta(d) => {
                assert_eq!(d.upserts.len(), 1);
                assert_eq!(d.upserts[0].position.unwrap().lon_deg, 2.0);
                assert!(d.coalesced);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn remove_cancels_pending_upsert() {
        let mut s = Subscription::new(1, 1, 100, 1000);
        s.make_snapshot(vec![]);
        s.enqueue_upsert(track("a", 1.0));
        s.enqueue_remove("a".into());
        match s.flush().unwrap() {
            ServerMessage::Delta(d) => {
                assert!(d.upserts.is_empty());
                assert_eq!(d.removes, vec!["a".to_string()]);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn overflowing_hard_cap_forces_resync_not_blocking() {
        let mut s = Subscription::new(1, 1, 2, 3);
        s.make_snapshot(vec![]);
        for i in 0..10 {
            s.enqueue_upsert(track(&format!("a{i}"), i as f64));
        }
        match s.flush().unwrap() {
            ServerMessage::ResyncRequired { .. } => {}
            other => panic!("expected resync, got {other:?}"),
        }
    }

    #[test]
    fn resync_invariant_old_snapshot_cannot_continue_with_deltas() {
        let mut s = Subscription::new(1, 1, 100, 1000);
        s.make_snapshot(vec![]); // snapshot id 1
        s.make_snapshot(vec![]); // snapshot id 2 (e.g. after a viewport change)
        // A client holding snapshot 1 may not resume with deltas.
        assert!(!s.can_resume_with_deltas(1));
        assert!(s.can_resume_with_deltas(2));
    }

    #[test]
    fn ack_advances_monotonically() {
        let mut s = Subscription::new(1, 1, 100, 1000);
        s.ack(5);
        s.ack(3); // stale ack ignored
        assert_eq!(s.acked_seq(), 5);
    }
}
