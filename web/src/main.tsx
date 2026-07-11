// Entry point. In this build the app starts in the honest empty/offline state
// until a source is connected. A real deployment injects a protocol client that
// feeds the TrackStore; wiring that transport is the next integration step.

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";

const root = createRoot(document.getElementById("root")!);
root.render(
  <StrictMode>
    <App
      tracks={[]}
      nowMs={Date.now()}
      connected={false}
      isReplay={false}
      sourceLabel="no source"
      timelineSpans={[]}
      timelineWindow={{ startMs: Date.now() - 60000, endMs: Date.now() }}
    />
  </StrictMode>,
);
