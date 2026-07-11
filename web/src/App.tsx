// Application shell (A32).
//
// Wires the globe, detail drawer, and timeline together over the protocol
// client. It handles the empty/offline/error states explicitly: when no feed
// is connected the UI says so rather than showing a blank globe that looks
// live, and when the store asks for a resync the shell shows that status.

import { useCallback, useMemo, useState } from "react";
import { GlobeView } from "./GlobeView";
import { DetailDrawer } from "./DetailDrawer";
import { Timeline, type TimelineSpan } from "./Timeline";
import type { TrackWire } from "./protocol";

export interface AppProps {
  // Injected so the shell is testable and the data source (live, local, or
  // labeled replay) is explicit. Replay sources set `isReplay` true.
  tracks: TrackWire[];
  nowMs: number;
  connected: boolean;
  isReplay: boolean;
  sourceLabel: string;
  timelineSpans: TimelineSpan[];
  timelineWindow: { startMs: number; endMs: number };
}

export function App(props: AppProps) {
  const { tracks, nowMs, connected, isReplay, sourceLabel } = props;
  const [selected, setSelected] = useState<string | null>(null);

  const selectedTrack = useMemo(
    () => tracks.find((t) => t.icao24 === selected) ?? null,
    [tracks, selected],
  );

  const onSelect = useCallback((icao24: string) => setSelected(icao24), []);

  return (
    <div className="app">
      <header className="topbar">
        <strong>Aeroscope</strong>
        <span className={`source-badge ${isReplay ? "replay" : "live"}`}>
          {isReplay ? "REPLAY" : connected ? "LIVE" : "OFFLINE"} · {sourceLabel}
        </span>
        <span className="track-count">{tracks.length} tracks in view</span>
      </header>

      <main className="globe-area">
        {!connected && tracks.length === 0 ? (
          <div className="empty-state" role="status">
            <p>No feed connected.</p>
            <p>
              Connect a local readsb receiver or load a labeled replay. Aeroscope
              does not invent aircraft when there is no data.
            </p>
          </div>
        ) : (
          <GlobeView tracks={tracks} nowMs={nowMs} onSelect={onSelect} />
        )}
      </main>

      <DetailDrawer
        track={selectedTrack}
        nowMs={nowMs}
        onClose={() => setSelected(null)}
      />

      <footer className="bottombar">
        <Timeline
          spans={props.timelineSpans}
          windowStartMs={props.timelineWindow.startMs}
          windowEndMs={props.timelineWindow.endMs}
        />
      </footer>
    </div>
  );
}
