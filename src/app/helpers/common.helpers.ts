import type { ProcessGraphSettings, StatePattern, TextAttributeOption } from '../ocel-wasm.service';
import type { DfEdgeFilterRequest } from '../models/filter.models';
import type { SummaryDisplayValue } from '../models/summary.models';

export function emptyGraphSettings(): ProcessGraphSettings {
  return {
    object_types: [],
    min_activity_frequency: 1,
    min_path_frequency: 1,
  };
}

export function correlationHeatStyle(correlation: number): string {
  const value = Number.isFinite(correlation) ? Math.max(-1, Math.min(1, correlation)) : 0;
  const strength = Math.abs(value);
  if (strength < 0.05) {
    return 'background: #f4f7f6; color: #263632;';
  }

  const hue = value >= 0 ? 166 : 22;
  const saturation = value >= 0 ? 48 : 68;
  const lightness = Math.round(96 - strength * 45);
  const color = strength >= 0.68 ? '#ffffff' : '#17221f';
  return `background: hsl(${hue} ${saturation}% ${lightness}%); color: ${color};`;
}

export function graphRequestJson(settings: ProcessGraphSettings): string {
  return JSON.stringify(settings);
}

export function errorToMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === 'string') {
    return error;
  }

  return 'Could not process the OCEL file.';
}

export function selectedPattern(patterns: StatePattern[], selectedId: string): StatePattern | null {
  return patterns.find((pattern) => pattern.id === selectedId) ?? patterns[0] ?? null;
}

export function emptySummaryValue(): SummaryDisplayValue {
  return {
    current: '0',
    filtered: false,
  };
}

export function toggleSelection(values: string[], value: string, checked: boolean): string[] {
  if (checked) {
    return values.includes(value) ? values : [...values, value];
  }

  return values.filter((candidate) => candidate !== value);
}

export function filterDescription(prefix: string, values: string[]): string {
  return values.length > 0 ? `${prefix}: ${values.join(', ')}` : `${prefix}: none`;
}

export function edgeLabel(edge: DfEdgeFilterRequest): string {
  return `${edge.source} -> ${edge.target}`;
}

export function edgeKey(edge: DfEdgeFilterRequest): string {
  return `${edge.source}\u0000${edge.target}`;
}

export function sameEdge(left: DfEdgeFilterRequest, right: DfEdgeFilterRequest): boolean {
  return left.source === right.source && left.target === right.target;
}

export function uniqueEdges(
  edge: DfEdgeFilterRequest,
  index: number,
  edges: DfEdgeFilterRequest[],
): boolean {
  return edges.findIndex((candidate) => sameEdge(candidate, edge)) === index;
}

export function graphMenuPosition(clientX: number, clientY: number): { x: number; y: number } {
  const width = typeof window === 'undefined' ? 1280 : window.innerWidth;
  const height = typeof window === 'undefined' ? 800 : window.innerHeight;
  return {
    x: Math.min(Math.max(clientX, 12), Math.max(12, width - 280)),
    y: Math.min(Math.max(clientY, 12), Math.max(12, height - 180)),
  };
}

export function textAttributeKey(attribute: Pick<TextAttributeOption, 'scope' | 'name'>): string {
  return `${attribute.scope}::${attribute.name}`;
}

export function clampInteger(value: string, min: number, max: number): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return min;
  }
  return Math.min(max, Math.max(min, parsed));
}

export function safeFilePart(value: string): string {
  return (
    value
      .trim()
      .replace(/[^A-Za-z0-9_-]+/g, '-')
      .replace(/^-|-$/g, '') || 'objects'
  );
}

export function withLeadingObjectTypeClause(query: string, leadingObjectType: string): string {
  const clause = `FOR LEADING OBJECT TYPE '${escapeSqlString(leadingObjectType)}'`;
  const stateHeader =
    /^\s*STATE\s+([A-Za-z_][A-Za-z0-9_-]*)(?:\s+FOR\s+LEADING\s+OBJECT\s+TYPE\s+(?:"(?:[^"]|"")*"|'(?:[^']|'')*'|[A-Za-z_][A-Za-z0-9_-]*))?\s+AS\s+CASE/im;

  if (stateHeader.test(query)) {
    return query.replace(stateHeader, (_match, attribute: string) => {
      const leadingWhitespace = query.match(/^\s*/)?.[0] ?? '';
      return `${leadingWhitespace}STATE ${attribute} ${clause} AS CASE`;
    });
  }

  return `STATE state ${clause} AS CASE\n  WHEN event.type IS NOT NULL THEN 'State'\nEND`;
}

export function escapeSqlString(value: string): string {
  return value.replace(/'/g, "''");
}
