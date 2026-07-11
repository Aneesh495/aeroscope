// The CesiumJS globe (A17, A18).
//
// Aircraft are drawn as a single batched PointPrimitiveCollection / Billboards,
// not one Entity per aircraft, so tens of thousands of glyphs stay on the GPU
// path. Color and glyph come from the freshness logic, and a stale or lost
// aircraft is pinned rather than animated. The default globe uses Cesium's
// built-in imagery so it is useful without a paid tile key (A34); a premium
// tile provider can be enabled behind configuration with visible attribution.

import { useEffect, useRef } from "react";
import * as Cesium from "cesium";
import type { TrackWire } from "./protocol";
import { freshnessFromAge, shouldAnimate } from "./freshness";

export interface GlobeViewProps {
  tracks: TrackWire[];
  nowMs: number;
  onSelect: (icao24: string) => void;
}

export function GlobeView({ tracks, nowMs, onSelect }: GlobeViewProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const viewerRef = useRef<Cesium.Viewer | null>(null);
  const pointsRef = useRef<Cesium.PointPrimitiveCollection | null>(null);
  const indexRef = useRef<Map<string, Cesium.PointPrimitive>>(new Map());

  // Initialize the viewer once. We disable the default photorealistic terrain
  // features that would otherwise hide a sparse feed behind pretty imagery.
  useEffect(() => {
    if (!containerRef.current || viewerRef.current) return;
    const viewer = new Cesium.Viewer(containerRef.current, {
      baseLayerPicker: false,
      geocoder: false,
      timeline: false,
      animation: false,
      sceneModePicker: true,
      homeButton: true,
      infoBox: false,
      selectionIndicator: false,
    });
    viewer.scene.globe.enableLighting = true; // day/night terminator (A18)
    const points = viewer.scene.primitives.add(
      new Cesium.PointPrimitiveCollection(),
    );
    pointsRef.current = points;
    viewerRef.current = viewer;

    // Selection by click, resolved to the nearest aircraft point.
    const handler = new Cesium.ScreenSpaceEventHandler(viewer.scene.canvas);
    handler.setInputAction((movement: { position: Cesium.Cartesian2 }) => {
      const picked = viewer.scene.pick(movement.position);
      if (picked && picked.id && typeof picked.id === "string") {
        onSelect(picked.id);
      }
    }, Cesium.ScreenSpaceEventType.LEFT_CLICK);

    return () => {
      handler.destroy();
      viewer.destroy();
      viewerRef.current = null;
    };
  }, [onSelect]);

  // Update the batched points when tracks change. We reuse PointPrimitives by
  // icao24 so steady-state updates mutate existing GPU records rather than
  // rebuilding the collection every frame.
  useEffect(() => {
    const points = pointsRef.current;
    if (!points) return;
    const index = indexRef.current;
    const seen = new Set<string>();

    for (const t of tracks) {
      if (!t.position) continue;
      const ageMs = t.event_time_ms ? nowMs - t.event_time_ms : null;
      const fresh = freshnessFromAge(ageMs, t.state);
      // Stale/lost aircraft are pinned at their last known position and dimmed;
      // we never animate a fabricated position.
      const animate = shouldAnimate(fresh.level);
      const pos = Cesium.Cartesian3.fromDegrees(
        t.position.lon_deg,
        t.position.lat_deg,
        t.position.alt_m,
      );
      const color = Cesium.Color.fromCssColorString(fresh.colorHex).withAlpha(
        animate ? 1.0 : 0.5,
      );
      seen.add(t.icao24);
      let p = index.get(t.icao24);
      if (!p) {
        p = points.add({ id: t.icao24, position: pos, pixelSize: 6, color });
        index.set(t.icao24, p);
      } else {
        p.position = pos;
        p.color = color;
      }
    }

    // Remove points no longer present.
    for (const [id, p] of index) {
      if (!seen.has(id)) {
        points.remove(p);
        index.delete(id);
      }
    }
  }, [tracks, nowMs]);

  return <div ref={containerRef} style={{ width: "100%", height: "100%" }} />;
}
