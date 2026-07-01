import type { FilterTimeBucket } from '../ocel-wasm.service';
import type { TimeRangeFilterRequest } from '../models/filter.models';
import type { TimeFilterCurve } from '../models/time.models';
import { formatDateTime, formatShortDate } from './time-format.helpers';
import { smoothPath } from './time-chart.helpers';

export function normalizeTimeRange(
  startMs: number | null,
  endMs: number | null,
  minMs?: number,
  maxMs?: number,
): TimeRangeFilterRequest | null {
  if (startMs === null && endMs === null) {
    return null;
  }

  let start = startMs ?? minMs;
  let end = endMs ?? maxMs;
  if (start === undefined && end === undefined) {
    return null;
  }
  if (start !== undefined && end !== undefined && start > end) {
    [start, end] = [end, start];
  }

  const normalized: TimeRangeFilterRequest = {};
  if (start !== undefined && start !== minMs) {
    normalized.start_ms = start;
  }
  if (end !== undefined && end !== maxMs) {
    normalized.end_ms = end;
  }

  return normalized.start_ms === undefined && normalized.end_ms === undefined ? null : normalized;
}

export function toDateTimeLocalInput(timeMs?: number): string {
  if (timeMs === undefined || !Number.isFinite(timeMs)) {
    return '';
  }
  const date = new Date(timeMs);
  const pad = (value: number) => value.toString().padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`;
}

export function fromDateTimeLocalInput(value: string): number | null {
  if (!value) {
    return null;
  }
  const timeMs = new Date(value).getTime();
  return Number.isFinite(timeMs) ? timeMs : null;
}

export function timeRangeLabel(range: TimeRangeFilterRequest): string {
  const start = range.start_ms !== undefined ? formatDateTime(range.start_ms) : 'start';
  const end = range.end_ms !== undefined ? formatDateTime(range.end_ms) : 'end';
  return `${start} -> ${end}`;
}

export function timeFilterCurve(
  buckets: FilterTimeBucket[],
  selectedStartMs: number | null,
  selectedEndMs: number | null,
): TimeFilterCurve | null {
  if (buckets.length === 0) {
    return null;
  }
  const width = 640;
  const height = 180;
  const padding = { left: 10, right: 10, top: 14, bottom: 32 };
  const maxCount = Math.max(1, ...buckets.map((bucket) => bucket.count));
  const points = buckets.map((bucket, index) => ({
    x:
      padding.left +
      (index / Math.max(buckets.length - 1, 1)) * (width - padding.left - padding.right),
    y:
      height - padding.bottom - (bucket.count / maxCount) * (height - padding.top - padding.bottom),
  }));
  const path = smoothPath(points);
  const baseline = height - padding.bottom;
  const rangeStartMs = buckets[0].start_ms;
  const rangeEndMs = buckets[buckets.length - 1].end_ms;
  const span = Math.max(rangeEndMs - rangeStartMs, 1);
  const selectedStart = selectedStartMs ?? rangeStartMs;
  const selectedEnd = selectedEndMs ?? rangeEndMs;
  const selectedStartX =
    padding.left +
    ((Math.min(selectedStart, selectedEnd) - rangeStartMs) / span) *
      (width - padding.left - padding.right);
  const selectedEndX =
    padding.left +
    ((Math.max(selectedStart, selectedEnd) - rangeStartMs) / span) *
      (width - padding.left - padding.right);
  const areaPath =
    path && points.length > 0
      ? `${path} L ${points[points.length - 1].x} ${baseline} L ${points[0].x} ${baseline} Z`
      : '';
  return {
    width,
    height,
    path,
    areaPath,
    startLabel: formatShortDate(buckets[0].start_ms),
    endLabel: formatShortDate(buckets[buckets.length - 1].end_ms),
    selectedStartX: Math.min(Math.max(selectedStartX, padding.left), width - padding.right),
    selectedEndX: Math.min(Math.max(selectedEndX, padding.left), width - padding.right),
    selectionTop: padding.top,
    selectionBottom: baseline,
  };
}
