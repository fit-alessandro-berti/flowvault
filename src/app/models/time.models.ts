import type { LifecycleStateBand, LifecycleStockPoint, TimePerformanceSample } from '../ocel-wasm.service';

export interface TimeFrequencySeries {
  state: string;
  color: string;
  path: string;
  areaPath: string;
  latest: number;
}

export interface TimeFrequencyChart {
  width: number;
  height: number;
  startLabel: string;
  endLabel: string;
  yTicks: number[];
  series: TimeFrequencySeries[];
}

export interface TimeFilterCurve {
  width: number;
  height: number;
  path: string;
  areaPath: string;
  startLabel: string;
  endLabel: string;
  selectedStartX: number;
  selectedEndX: number;
  selectionTop: number;
  selectionBottom: number;
}

export interface PerformanceSpectrumLine {
  sample: TimePerformanceSample;
  path: string;
  color: string;
  opacity: number;
}

export interface PerformanceSpectrumChart {
  width: number;
  height: number;
  lines: PerformanceSpectrumLine[];
  laneLabels: { label: string; y: number }[];
  xTicks: { label: string; x: number }[];
}

export interface TransitionMatrixCell {
  fromState: string;
  toState: string;
  count: number;
  objectCount: number;
  medianDurationMs?: number;
  intensity: number;
}

export interface TransitionMatrixView {
  states: string[];
  maxCount: number;
  rows: Array<{
    state: string;
    cells: TransitionMatrixCell[];
  }>;
}

export interface LifecycleBandView extends LifecycleStateBand {
  x: number;
  width: number;
}

export interface LifecycleStockSeries {
  name: string;
  color: string;
  path: string;
  points: Array<LifecycleStockPoint & { x: number; y: number }>;
}

export interface LifecycleTimelineView {
  width: number;
  height: number;
  startLabel: string;
  endLabel: string;
  bands: LifecycleBandView[];
  stockSeries: LifecycleStockSeries[];
  yMin: number;
  yMax: number;
}
