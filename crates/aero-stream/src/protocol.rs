//! The Aeroscope streaming protocol wire types (A10, A31).
//!
//! The protocol is versioned. A client announces the versions it supports; the
//! server picks the highest shared version or refuses. Messages carry a
//! monotonically increasing per-subscription sequence number so a client can
//! detect a gap and request a resync. The core state-transfer model is:
//!
//!   Snapshot  -> full set of tracks in view, with a snapshot id and base seq
//!   Delta     -> adds/updates/removes relative to a specific snapshot id
//!
//! A delta names the snapshot id it builds on. A client that reconnects with an
//! old snapshot id must be sent a fresh snapshot, never a delta, so an old
//! delta can never be applied to a newer base. This is the resync invariant.

use aero_fusion::{SelectionReason, TrackState};
use aero_geo::Geodetic;
use serde::{Deserialize, Serialize};

/// Current protocol version. Bump on any breaking wire change.
pub const PROTOCOL_VERSION: u16 = 1;

/// A compact per-track payload sent on the wire. Keeps provenance so the client
/// can label freshness and source without a second request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackWire {
    pub icao24: String,
    pub position: Option<Geodetic>,
    pub event_time_ms: i64,
    pub state: TrackState,
    pub selected_source: Option<String>,
    pub selection_reason: SelectionReason,
}

/// A monotonic sequence number for one subscription.
pub type Seq = u64;

/// Identifies a snapshot so deltas can reference their base.
pub type SnapshotId = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub snapshot_id: SnapshotId,
    /// Sequence number of this snapshot message.
    pub seq: Seq,
    pub tracks: Vec<TrackWire>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delta {
    /// The snapshot this delta is relative to.
    pub base_snapshot_id: SnapshotId,
    pub seq: Seq,
    pub upserts: Vec<TrackWire>,
    /// icao24 hexes that left the viewport or ended.
    pub removes: Vec<String>,
    /// Whether any live state was coalesced (multiple updates collapsed to the
    /// latest) for a slow consumer. Coalescing live state never touches
    /// retained history; this flag makes the behavior explicit (A11).
    pub coalesced: bool,
}

/// Messages from server to client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// Negotiated protocol version and the subscription id.
    Welcome { version: u16, subscription_id: u64 },
    Snapshot(Snapshot),
    Delta(Delta),
    /// Sent when the server must drop a slow consumer's backlog; the client
    /// should expect a fresh snapshot next and discard its state.
    ResyncRequired { reason: String },
    /// Version negotiation failed.
    Unsupported { server_versions: Vec<u16> },
}

/// Messages from client to server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Client announces supported versions and the viewport it wants.
    Subscribe {
        supported_versions: Vec<u16>,
        south_lat: f64,
        north_lat: f64,
        west_lon: f64,
        east_lon: f64,
    },
    /// Acknowledge receipt up to and including this sequence number.
    Ack { seq: Seq },
    /// Reconnect declaring the last snapshot id and seq the client holds.
    Resume { last_snapshot_id: SnapshotId, last_seq: Seq },
    Unsubscribe,
}

/// Negotiate the highest version the server and client both support.
pub fn negotiate(server: &[u16], client: &[u16]) -> Option<u16> {
    server
        .iter()
        .filter(|v| client.contains(v))
        .copied()
        .max()
}

/// Forward-compatibility: unknown fields in a client message must not crash the
/// server. serde_json with default-deny is strict, so we parse leniently by
/// ignoring unknown keys (the structs above are not `deny_unknown_fields`).
pub fn parse_client_message(bytes: &[u8]) -> Result<ClientMessage, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiation_picks_highest_shared() {
        assert_eq!(negotiate(&[1, 2, 3], &[2, 3, 4]), Some(3));
        assert_eq!(negotiate(&[1], &[2, 3]), None);
    }

    #[test]
    fn unknown_fields_are_tolerated() {
        // A newer client sends an extra field; server on v1 must still parse.
        let json = br#"{"type":"Ack","seq":5,"future_field":true}"#;
        let msg = parse_client_message(json).unwrap();
        assert_eq!(msg, ClientMessage::Ack { seq: 5 });
    }

    #[test]
    fn snapshot_and_delta_roundtrip_json() {
        let snap = ServerMessage::Snapshot(Snapshot {
            snapshot_id: 1,
            seq: 1,
            tracks: vec![TrackWire {
                icao24: "4ca7b3".into(),
                position: Some(Geodetic::new(51.0, 0.0, 10000.0)),
                event_time_ms: 1000,
                state: TrackState::Observed,
                selected_source: Some("rx".into()),
                selection_reason: SelectionReason::SingleSource,
            }],
        });
        let bytes = serde_json::to_vec(&snap).unwrap();
        let back: ServerMessage = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(snap, back);
    }
}
