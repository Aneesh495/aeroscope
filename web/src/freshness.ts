// Freshness and honesty logic for the UI (A19, A20, G07).
//
// The globe must never show a stale or missing observation as if it were a
// fresh live aircraft. This module turns an observation age and a track state
// into an explicit, color-accessible freshness label. Colors are paired with
// text and a shape hint so the encoding does not rely on color alone.

import type { TrackState } from "./protocol";

export type FreshnessLevel = "fresh" | "aging" | "stale" | "lost" | "unknown";

export interface FreshnessLabel {
  level: FreshnessLevel;
  text: string;
  // A hue for sighted users, always paired with the text above so the status
  // is never conveyed by color alone (accessibility, A32).
  colorHex: string;
  // A glyph hint for a non-color channel.
  glyph: "dot" | "ring" | "hollow" | "cross" | "question";
}

// Declared age thresholds mirror the Rust LifecycleConfig defaults.
const FRESH_MS = 5_000;
const AGING_MS = 20_000;
const STALE_MS = 60_000;

export function freshnessFromAge(
  ageMs: number | null,
  state: TrackState,
): FreshnessLabel {
  if (ageMs === null) {
    return {
      level: "unknown",
      text: "age unknown",
      colorHex: "#9aa0a6",
      glyph: "question",
    };
  }
  if (state === "Lost" || ageMs > STALE_MS) {
    return { level: "lost", text: "lost", colorHex: "#b0b0b0", glyph: "cross" };
  }
  if (state === "Stale" || ageMs > AGING_MS) {
    return {
      level: "stale",
      text: `stale ${Math.round(ageMs / 1000)}s`,
      colorHex: "#d9a441",
      glyph: "hollow",
    };
  }
  if (ageMs > FRESH_MS) {
    return {
      level: "aging",
      text: `${Math.round(ageMs / 1000)}s old`,
      colorHex: "#4f93ce",
      glyph: "ring",
    };
  }
  return { level: "fresh", text: "live", colorHex: "#3fb950", glyph: "dot" };
}

// Whether the symbol should be drawn as moving. Only fresh and aging tracks
// move; stale and lost are pinned so the UI never animates a fabricated
// position (honesty invariant).
export function shouldAnimate(level: FreshnessLevel): boolean {
  return level === "fresh" || level === "aging";
}
