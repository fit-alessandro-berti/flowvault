export function validStateSelection(
  current: string,
  states: string[],
  fallback: string,
  excluded = '',
): string {
  if (current && current !== excluded && states.includes(current)) {
    return current;
  }
  if (fallback && fallback !== excluded && states.includes(fallback)) {
    return fallback;
  }
  return states.find((state) => state !== excluded) ?? '';
}

export function formatDateTime(timeMs: number): string {
  if (!Number.isFinite(timeMs)) {
    return '-';
  }
  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(timeMs));
}

export function formatShortDate(timeMs: number): string {
  if (!Number.isFinite(timeMs)) {
    return '-';
  }
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: '2-digit',
    year: 'numeric',
  }).format(new Date(timeMs));
}

export function formatDuration(durationMs?: number | null): string {
  if (durationMs === undefined || durationMs === null || !Number.isFinite(durationMs)) {
    return '-';
  }
  const absolute = Math.max(0, durationMs);
  const minutes = absolute / 60_000;
  if (minutes < 1) {
    return `${Math.round(absolute / 1000)}s`;
  }
  if (minutes < 90) {
    return `${round(minutes)}m`;
  }
  const hours = minutes / 60;
  if (hours < 48) {
    return `${round(hours)}h`;
  }
  return `${round(hours / 24)}d`;
}

export function round(value: number): number {
  return Math.round(value * 100) / 100;
}
