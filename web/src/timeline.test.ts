import { describe, it, expect } from "vitest";
import { pointsToSpans } from "./Timeline";

describe("pointsToSpans", () => {
  it("marks a long interval as a gap, not an observed span", () => {
    const spans = pointsToSpans(
      [
        { ms: 0, observed: true },
        { ms: 1000, observed: true },
        { ms: 20000, observed: true }, // 19s jump -> gap
        { ms: 21000, observed: true },
      ],
      5000,
    );
    const kinds = spans.map((s) => s.kind);
    expect(kinds).toEqual(["observed", "gap", "observed"]);
  });

  it("marks a non-observed point's span as stale", () => {
    const spans = pointsToSpans(
      [
        { ms: 0, observed: false },
        { ms: 1000, observed: true },
      ],
      5000,
    );
    expect(spans[0].kind).toBe("stale");
  });

  it("carries the source label through", () => {
    const spans = pointsToSpans(
      [
        { ms: 0, observed: true, source: "rx" },
        { ms: 1000, observed: true, source: "rx" },
      ],
      5000,
    );
    expect(spans[0].source).toBe("rx");
  });
});
