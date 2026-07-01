import type { TimeFrequencyBucket, TimePerformanceSpectrum } from '../ocel-wasm.service';
import type { PerformanceSpectrumChart, TimeFrequencyChart } from '../models/time.models';
import { formatDuration, formatShortDate, round } from './time-format.helpers';

export function timeFrequencyChart(
  buckets: TimeFrequencyBucket[],
  states: string[],
): TimeFrequencyChart | null {
  if (buckets.length === 0 || states.length === 0) {
    return null;
  }
  const width = 790;
  const height = 270;
  const plot = { left: 48, right: 740, top: 28, bottom: 220 };
  const colorByState = new Map(
    states.map((state, index) => [state, CHART_COLORS[index % CHART_COLORS.length]]),
  );
  const percentageByBucket = buckets.map(
    (bucket) => new Map(bucket.percentages.map((entry) => [entry.state, entry.percentage])),
  );
  const series = states.map((state) => {
    const points = buckets.map((bucket, index) => {
      const ratio = index / Math.max(buckets.length - 1, 1);
      const percentage = percentageByBucket[index].get(state) ?? 0;
      return {
        x: plot.left + ratio * (plot.right - plot.left),
        y: plot.bottom - (percentage / 100) * (plot.bottom - plot.top),
      };
    });
    const path = smoothPath(points);
    const areaPath =
      path && points.length > 0
        ? `${path} L ${points[points.length - 1].x} ${plot.bottom} L ${points[0].x} ${plot.bottom} Z`
        : '';
    return {
      state,
      color: colorByState.get(state) ?? CHART_COLORS[0],
      path,
      areaPath,
      latest: percentageByBucket[percentageByBucket.length - 1].get(state) ?? 0,
    };
  });

  return {
    width,
    height,
    startLabel: formatShortDate(buckets[0].start_ms),
    endLabel: formatShortDate(buckets[buckets.length - 1].end_ms),
    yTicks: [0, 25, 50, 75, 100],
    series,
  };
}

export function performanceSpectrumChart(
  performance: TimePerformanceSpectrum,
  eventMinMs: number,
  eventMaxMs: number,
): PerformanceSpectrumChart | null {
  const width = 800;
  const height = 230;
  const samples = performance.samples;
  const laneLabels = performance.roundtrip
    ? [performance.from_state, performance.to_state, performance.from_state]
    : [performance.from_state, performance.to_state];
  const laneY = performance.roundtrip ? [44, 102, 160] : [62, 144];
  if (samples.length === 0) {
    return {
      width,
      height,
      lines: [],
      laneLabels: laneLabels.map((label, index) => ({ label, y: laneY[index] })),
      xTicks: [],
    };
  }

  const minTime = Number.isFinite(eventMinMs) ? eventMinMs : samples[0].start_time_ms;
  const maxTime = Number.isFinite(eventMaxMs)
    ? eventMaxMs
    : samples[samples.length - 1].middle_time_ms;
  const timeSpan = Math.max(maxTime - minTime, 1);
  const plot = { left: 118, right: 748 };
  const xForTime = (timeMs: number) =>
    plot.left + ((timeMs - minTime) / timeSpan) * (plot.right - plot.left);

  const durations = [...samples]
    .map((sample) => sample.duration_ms)
    .sort((left, right) => left - right);
  const q1 = durations[Math.floor(durations.length * 0.25)] ?? 0;
  const q2 = durations[Math.floor(durations.length * 0.5)] ?? q1;
  const q3 = durations[Math.floor(durations.length * 0.75)] ?? q2;

  const lines = samples.map((sample) => {
    const points = [
      `${round(xForTime(sample.start_time_ms))},${laneY[0]}`,
      `${round(xForTime(sample.middle_time_ms))},${laneY[1]}`,
    ];
    if (performance.roundtrip && sample.end_time_ms !== undefined) {
      points.push(`${round(xForTime(sample.end_time_ms))},${laneY[2]}`);
    }
    return {
      sample,
      path: `M ${points.join(' L ')}`,
      color: durationQuartileColor(sample.duration_ms, q1, q2, q3),
      opacity: 0.58,
    };
  });

  return {
    width,
    height,
    lines,
    laneLabels: laneLabels.map((label, index) => ({ label, y: laneY[index] })),
    xTicks: [
      { label: formatShortDate(minTime), x: plot.left },
      { label: formatShortDate(maxTime), x: plot.right },
    ],
  };
}

export function durationQuartileColor(durationMs: number, q1: number, q2: number, q3: number): string {
  if (durationMs <= q1) {
    return '#1f5aa6';
  }
  if (durationMs <= q2) {
    return '#4fa3d1';
  }
  if (durationMs <= q3) {
    return '#f0a23a';
  }
  return '#c83737';
}

export function smoothPath(points: { x: number; y: number }[]): string {
  if (points.length === 0) {
    return '';
  }
  if (points.length === 1) {
    return `M ${round(points[0].x)} ${round(points[0].y)}`;
  }

  const commands = [`M ${round(points[0].x)} ${round(points[0].y)}`];
  for (let index = 0; index < points.length - 1; index += 1) {
    const previous = points[Math.max(0, index - 1)];
    const current = points[index];
    const next = points[index + 1];
    const following = points[Math.min(points.length - 1, index + 2)];
    const cp1x = current.x + (next.x - previous.x) / 6;
    const cp1y = current.y + (next.y - previous.y) / 6;
    const cp2x = next.x - (following.x - current.x) / 6;
    const cp2y = next.y - (following.y - current.y) / 6;
    commands.push(
      `C ${round(cp1x)} ${round(cp1y)}, ${round(cp2x)} ${round(cp2y)}, ${round(next.x)} ${round(
        next.y,
      )}`,
    );
  }
  return commands.join(' ');
}

export const CHART_COLORS = [
  '#1d4f49',
  '#4678a0',
  '#b45f1a',
  '#7b4fa3',
  '#2f7d3d',
  '#a33f5f',
  '#5661a8',
  '#8b6b23',
];
