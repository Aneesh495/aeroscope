//! The Aeroscope streaming protocol and subscription delivery (A10, A11).
//!
//! A versioned, compact protocol delivers viewport state to many subscribers
//! as acknowledged snapshots and deltas. Each subscription bounds its own work:
//! sequence numbers let clients detect gaps, coalescing collapses a slow
//! consumer's backlog to the latest state, and a hard cap forces an explicit
//! resync rather than blocking ingestion or growing memory. The resync
//! invariant guarantees an old delta can never be applied to a newer snapshot.

pub mod protocol;
pub mod subscription;

pub use protocol::{
    negotiate, parse_client_message, ClientMessage, Delta, Seq, ServerMessage, Snapshot,
    SnapshotId, TrackWire, PROTOCOL_VERSION,
};
pub use subscription::{QueuePolicy, Subscription};
