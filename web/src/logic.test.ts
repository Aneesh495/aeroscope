import { describe, it, expect } from "vitest";
import { TrackStore, type ServerMessage } from "./protocol";
import { freshnessFromAge, shouldAnimate } from "./freshness";
import { computeCoverage } from "./coverage";

function snapshot(id: number, seq: number, icaos: string[]): ServerMessage {
  return {
    type: "Snapshot",
    snapshot_id: id,
    seq,
    tracks: icaos.map((h) => ({
      icao24: h,
      position: { lat_deg: 0, lon_deg: 0, alt_m: 10000 },
      event_time_ms: 1000,
      state: "Observed",
      selected_source: "rx",
      selection_reason: "SingleSource",
    })),
  };
}

describe("TrackStore", () => {
  it("applies a snapshot", () => {
    const s = new TrackStore();
    expect(s.apply(snapshot(1, 1, ["a", "b"]))).toBeNull();
    expect(s.size).toBe(2);
    expect(s.currentSnapshotId).toBe(1);
  });

  it("applies a sequential delta", () => {
    const s = new TrackStore();
    s.apply(snapshot(1, 1, ["a"]));
    const r = s.apply({
      type: "Delta",
      base_snapshot_id: 1,
      seq: 2,
      upserts: [
        {
          icao24: "b",
          position: { lat_deg: 1, lon_deg: 1, alt_m: 10000 },
          event_time_ms: 2000,
          state: "Observed",
          selected_source: "rx",
          selection_reason: "SingleSource",
        },
      ],
      removes: ["a"],
      coalesced: false,
    });
    expect(r).toBeNull();
    expect(s.size).toBe(1);
    expect(s.get("b")).toBeDefined();
    expect(s.get("a")).toBeUndefined();
  });

  it("rejects a delta on a stale snapshot (resync invariant)", () => {
    const s = new TrackStore();
    s.apply(snapshot(2, 1, ["a"]));
    const r = s.apply({
      type: "Delta",
      base_snapshot_id: 1, // old snapshot
      seq: 2,
      upserts: [],
      removes: [],
      coalesced: false,
    });
    expect(r).toMatch(/base 1/);
    expect(s.resyncNeeded).toBe(true);
  });

  it("detects a sequence gap", () => {
    const s = new TrackStore();
    s.apply(snapshot(1, 1, ["a"]));
    const r = s.apply({
      type: "Delta",
      base_snapshot_id: 1,
      seq: 5, // expected 2
      upserts: [],
      removes: [],
      coalesced: false,
    });
    expect(r).toMatch(/sequence gap/);
    expect(s.resyncNeeded).toBe(true);
  });
});

describe("freshness", () => {
  it("labels fresh, aging, stale, lost, unknown", () => {
    expect(freshnessFromAge(1000, "Observed").level).toBe("fresh");
    expect(freshnessFromAge(10000, "Observed").level).toBe("aging");
    expect(freshnessFromAge(30000, "Stale").level).toBe("stale");
    expect(freshnessFromAge(90000, "Lost").level).toBe("lost");
    expect(freshnessFromAge(null, "Observed").level).toBe("unknown");
  });

  it("only animates fresh/aging so a stale position is never moved", () => {
    expect(shouldAnimate("fresh")).toBe(true);
    expect(shouldAnimate("aging")).toBe(true);
    expect(shouldAnimate("stale")).toBe(false);
    expect(shouldAnimate("lost")).toBe(false);
    expect(shouldAnimate("unknown")).toBe(false);
  });

  it("pairs status with text and a non-color glyph", () => {
    const l = freshnessFromAge(1000, "Observed");
    expect(l.text.length).toBeGreaterThan(0);
    expect(l.glyph).toBe("dot");
  });
});

describe("coverage", () => {
  it("bins observations and flags density is not traffic", () => {
    const c = computeCoverage([
      { lat: 1, lon: 1 },
      { lat: 2, lon: 2 }, // same 5-deg bin as (1,1)
      { lat: 50, lon: 50 },
    ]);
    expect(c.observedCellCount).toBe(2);
    expect(c.totalObservations).toBe(3);
    expect(c.note).toMatch(/not an absolute count/i);
  });
});
