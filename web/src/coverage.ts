// Coverage computation for the UI (A23, G12).
//
// Coverage is measured from actual observations and source metadata, never
// asserted. We bin observed positions into a coarse grid and report observed
// cell density separately from any absolute traffic estimate, because sampled
// density is not the same as how many aircraft truly exist in a region.

export interface CoverageCell {
  latBin: number;
  lonBin: number;
  observationCount: number;
}

export interface CoverageSummary {
  cells: CoverageCell[];
  observedCellCount: number;
  totalObservations: number;
  // Explicit disclaimer the UI must show so density is not read as traffic.
  note: string;
}

const BIN_DEG = 5;

export function computeCoverage(
  positions: { lat: number; lon: number }[],
): CoverageSummary {
  const map = new Map<string, CoverageCell>();
  for (const p of positions) {
    const latBin = Math.floor(p.lat / BIN_DEG) * BIN_DEG;
    const lonBin = Math.floor(p.lon / BIN_DEG) * BIN_DEG;
    const key = `${latBin},${lonBin}`;
    const cell = map.get(key) ?? { latBin, lonBin, observationCount: 0 };
    cell.observationCount += 1;
    map.set(key, cell);
  }
  const cells = [...map.values()];
  return {
    cells,
    observedCellCount: cells.length,
    totalObservations: positions.length,
    note: "Observed sample density. This is not an absolute count of aircraft and does not imply coverage of unsampled regions.",
  };
}
