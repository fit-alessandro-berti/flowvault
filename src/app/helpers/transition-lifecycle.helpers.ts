import type { LifecycleStockPoint, ObjectLifecycleDetail, StateTransitionKpiResult } from '../ocel-wasm.service';
import type { LifecycleTimelineView, TransitionMatrixView } from '../models/time.models';
import { formatShortDate, round } from './time-format.helpers';
import { CHART_COLORS, smoothPath } from './time-chart.helpers';

export function transitionMatrixView(analysis: StateTransitionKpiResult): TransitionMatrixView {
  const maxCount = Math.max(...analysis.transitions.map((transition) => transition.count), 0);
  const byPair = new Map(
    analysis.transitions.map((transition) => [
      `${transition.from_state}\u0000${transition.to_state}`,
      transition,
    ]),
  );
  const rows = analysis.states.map((fromState) => ({
    state: fromState,
    cells: analysis.states.map((toState) => {
      const transition = byPair.get(`${fromState}\u0000${toState}`);
      const count = transition?.count ?? 0;
      return {
        fromState,
        toState,
        count,
        objectCount: transition?.object_count ?? 0,
        medianDurationMs: transition?.median_duration_ms,
        intensity: maxCount > 0 ? count / maxCount : 0,
      };
    }),
  }));

  return {
    states: analysis.states,
    maxCount,
    rows,
  };
}

export function lifecycleTimelineView(detail: ObjectLifecycleDetail): LifecycleTimelineView | null {
  const minTime = detail.event_min_ms ?? detail.events[0]?.time_ms;
  const maxTime = detail.event_max_ms ?? detail.events[detail.events.length - 1]?.time_ms;
  if (minTime === undefined || maxTime === undefined) {
    return null;
  }

  const width = 820;
  const height = detail.stock_points.length > 0 ? 260 : 150;
  const plot = { left: 58, right: 790, top: 32, bandTop: 34, bandHeight: 48, stockTop: 112, bottom: 220 };
  const timeSpan = Math.max(maxTime - minTime, 1);
  const xForTime = (timeMs: number) =>
    plot.left + ((timeMs - minTime) / timeSpan) * (plot.right - plot.left);

  const bands = detail.state_bands.map((band) => {
    const x = xForTime(band.start_time_ms);
    const endX = xForTime(band.end_time_ms);
    return {
      ...band,
      x,
      width: Math.max(endX - x, 3),
    };
  });

  const values = detail.stock_points.map((point) => point.value);
  let yMin = Math.min(...values, 0);
  let yMax = Math.max(...values, 1);
  if (Math.abs(yMax - yMin) <= Number.EPSILON) {
    yMin -= 1;
    yMax += 1;
  }
  const yForValue = (value: number) =>
    plot.bottom - ((value - yMin) / (yMax - yMin)) * (plot.bottom - plot.stockTop);
  const pointsByName = new Map<string, LifecycleStockPoint[]>();
  for (const point of detail.stock_points) {
    const points = pointsByName.get(point.name) ?? [];
    points.push(point);
    pointsByName.set(point.name, points);
  }
  const stockSeries = [...pointsByName.entries()].map(([name, points], index) => {
    const positioned = points
      .sort((left, right) => left.time_ms - right.time_ms)
      .map((point) => ({
        ...point,
        x: xForTime(point.time_ms),
        y: yForValue(point.value),
      }));
    return {
      name,
      color: CHART_COLORS[index % CHART_COLORS.length],
      path: smoothPath(positioned),
      points: positioned,
    };
  });

  return {
    width,
    height,
    startLabel: formatShortDate(minTime),
    endLabel: formatShortDate(maxTime),
    bands,
    stockSeries,
    yMin,
    yMax,
  };
}
