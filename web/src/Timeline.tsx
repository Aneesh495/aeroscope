// Observation timeline (A20).
//
// Renders a track's recent history as distinct spans: observed, interpolated,
// stale, and source-change markers. The key honesty property is that an unknown
// or gap span is drawn as a gap, never as a continuous observed line.

export interface TimelineSpan {
  startMs: number;
  endMs: number;
  kind: "observed" | "interpolated" | "stale" | "gap";
  source?: string;
}

export interface TimelineProps {
  spans: TimelineSpan[];
  windowStartMs: number;
  windowEndMs: number;
}

const KIND_COLOR: Record<TimelineSpan["kind"], string> = {
  observed: "#3fb950",
  interpolated: "#4f93ce",
  stale: "#d9a441",
  gap: "transparent",
};

export function Timeline({ spans, windowStartMs, windowEndMs }: TimelineProps) {
  const total = Math.max(1, windowEndMs - windowStartMs);
  return (
    <div className="timeline" role="img" aria-label="Observation timeline">
      <div className="timeline-track">
        {spans.map((s, i) => {
          const left = ((s.startMs - windowStartMs) / total) * 100;
          const width = ((s.endMs - s.startMs) / total) * 100;
          const isGap = s.kind === "gap";
          return (
            <div
              key={i}
              className={`timeline-span ${s.kind}`}
              title={`${s.kind}${s.source ? ` (${s.source})` : ""}`}
              style={{
                left: `${left}%`,
                width: `${width}%`,
                background: KIND_COLOR[s.kind],
                // A gap is drawn as an empty, dashed region so it reads as
                // "no data here", not as an observed path.
                border: isGap ? "1px dashed #666" : "none",
              }}
            />
          );
        })}
      </div>
      <div className="timeline-legend">
        <span style={{ color: KIND_COLOR.observed }}>observed</span>
        <span style={{ color: KIND_COLOR.interpolated }}>interpolated</span>
        <span style={{ color: KIND_COLOR.stale }}>stale</span>
        <span>gap (no data)</span>
      </div>
    </div>
  );
}

// Pure helper (tested): turn a sequence of (time, observed) points into spans,
// inserting gap spans where the inter-point interval exceeds `gapMs`.
export function pointsToSpans(
  points: { ms: number; observed: boolean; source?: string }[],
  gapMs: number,
): TimelineSpan[] {
  const spans: TimelineSpan[] = [];
  for (let i = 0; i < points.length - 1; i++) {
    const a = points[i];
    const b = points[i + 1];
    const dt = b.ms - a.ms;
    if (dt > gapMs) {
      spans.push({ startMs: a.ms, endMs: b.ms, kind: "gap" });
    } else {
      spans.push({
        startMs: a.ms,
        endMs: b.ms,
        kind: a.observed ? "observed" : "stale",
        source: a.source,
      });
    }
  }
  return spans;
}
