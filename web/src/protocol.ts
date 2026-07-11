// TypeScript mirror of the Aeroscope streaming protocol (crates/aero-stream).
//
// These types match the Rust `ServerMessage`/`ClientMessage` wire shapes so the
// client and server stay in sync. The client applies snapshots and deltas with
// the same resync invariant the server enforces: a delta may only be applied on
// top of the snapshot id it names, so an old delta can never corrupt a newer
// snapshot.

export const PROTOCOL_VERSION = 1;

export type TrackState =
  | "Observed"
  | "Interpolated"
  | "Extrapolated"
  | "Stale"
  | "Lost"
  | "Ended";

export interface Geodetic {
  lat_deg: number;
  lon_deg: number;
  alt_m: number;
}

export interface TrackWire {
  icao24: string;
  position: Geodetic | null;
  event_time_ms: number;
  state: TrackState;
  selected_source: string | null;
  selection_reason: string;
}

export interface Snapshot {
  snapshot_id: number;
  seq: number;
  tracks: TrackWire[];
}

export interface Delta {
  base_snapshot_id: number;
  seq: number;
  upserts: TrackWire[];
  removes: string[];
  coalesced: boolean;
}

export type ServerMessage =
  | { type: "Welcome"; version: number; subscription_id: number }
  | ({ type: "Snapshot" } & Snapshot)
  | ({ type: "Delta" } & Delta)
  | { type: "ResyncRequired"; reason: string }
  | { type: "Unsupported"; server_versions: number[] };

// Client-side view state built from snapshots and deltas.
export class TrackStore {
  private tracks = new Map<string, TrackWire>();
  private snapshotId = 0;
  private lastSeq = 0;
  private needsResync = false;

  get size(): number {
    return this.tracks.size;
  }
  get currentSnapshotId(): number {
    return this.snapshotId;
  }
  get resyncNeeded(): boolean {
    return this.needsResync;
  }

  all(): TrackWire[] {
    return [...this.tracks.values()];
  }
  get(icao24: string): TrackWire | undefined {
    return this.tracks.get(icao24);
  }

  // Apply a server message. Returns a short reason string when a gap or resync
  // is detected so the caller can request a fresh snapshot.
  apply(msg: ServerMessage): string | null {
    switch (msg.type) {
      case "Snapshot": {
        this.tracks.clear();
        for (const t of msg.tracks) this.tracks.set(t.icao24, t);
        this.snapshotId = msg.snapshot_id;
        this.lastSeq = msg.seq;
        this.needsResync = false;
        return null;
      }
      case "Delta": {
        // Resync invariant: reject a delta that does not build on the current
        // snapshot. This is what stops an old delta from corrupting state.
        if (msg.base_snapshot_id !== this.snapshotId) {
          this.needsResync = true;
          return `delta base ${msg.base_snapshot_id} != snapshot ${this.snapshotId}`;
        }
        // Gap detection via sequence numbers.
        if (msg.seq !== this.lastSeq + 1) {
          this.needsResync = true;
          return `sequence gap: expected ${this.lastSeq + 1}, got ${msg.seq}`;
        }
        for (const t of msg.upserts) this.tracks.set(t.icao24, t);
        for (const r of msg.removes) this.tracks.delete(r);
        this.lastSeq = msg.seq;
        return null;
      }
      case "ResyncRequired": {
        this.needsResync = true;
        return msg.reason;
      }
      default:
        return null;
    }
  }
}
