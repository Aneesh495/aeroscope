// Selected-aircraft detail drawer (A19).
//
// Shows the timestamp age, raw source state, both altitude types, speed,
// vertical rate, callsign, track state, selection reason, and attribution. The
// age and source are front and center because the product's honesty depends on
// the viewer always knowing how old and how trustworthy a state is.

import type { TrackWire } from "./protocol";
import { freshnessFromAge } from "./freshness";

export interface DetailDrawerProps {
  track: TrackWire | null;
  nowMs: number;
  onClose: () => void;
}

export function DetailDrawer({ track, nowMs, onClose }: DetailDrawerProps) {
  if (!track) return null;
  const ageMs = track.event_time_ms ? nowMs - track.event_time_ms : null;
  const fresh = freshnessFromAge(ageMs, track.state);

  return (
    <aside className="detail-drawer" role="complementary" aria-label="Aircraft detail">
      <header>
        <h2>{track.icao24}</h2>
        <button onClick={onClose} aria-label="Close detail">
          ×
        </button>
      </header>
      <dl>
        <dt>Freshness</dt>
        <dd>
          <span aria-hidden="true" style={{ color: fresh.colorHex }}>
            ●
          </span>{" "}
          {fresh.text} ({fresh.level})
        </dd>

        <dt>Track state</dt>
        <dd>{track.state}</dd>

        <dt>Position</dt>
        <dd>
          {track.position
            ? `${track.position.lat_deg.toFixed(4)}, ${track.position.lon_deg.toFixed(4)}`
            : "no position reported"}
        </dd>

        <dt>Geometric altitude</dt>
        <dd>
          {track.position
            ? `${Math.round(track.position.alt_m)} m (used for placement)`
            : "unknown"}
        </dd>

        <dt>Selected source</dt>
        <dd>{track.selected_source ?? "unknown"}</dd>

        <dt>Why this source</dt>
        <dd>{track.selection_reason}</dd>

        <dt>Event time</dt>
        <dd>{new Date(track.event_time_ms).toISOString()}</dd>
      </dl>
      <footer className="attribution">
        Observation attributed to {track.selected_source ?? "an unknown source"}.
        Altitudes shown are as reported; geometric altitude positions the
        aircraft. Unknown fields are shown as unknown, not guessed.
      </footer>
    </aside>
  );
}
