import type { PatternTab } from './pattern.models';

export interface FilterRequest {
  event_types: string[];
  object_types: string[];
  df_nodes?: string[];
  df_edges?: DfEdgeFilterRequest[];
  time_range?: TimeRangeFilterRequest;
  text_attributes?: TextAttributeFilterRequest[];
  patterns?: PatternFilterRequest[];
}

export interface DfEdgeFilterRequest {
  source: string;
  target: string;
}

export interface TimeRangeFilterRequest {
  start_ms?: number;
  end_ms?: number;
}

export interface TextAttributeFilterRequest {
  scope: 'event' | 'object';
  name: string;
  values: string[];
}

export interface PatternFilterRequest {
  family: PatternTab;
  leading_object_type: string;
  state?: string;
  from_state?: string;
  to_state?: string;
  sequence: string[];
  eo_edges: PatternEdgeFilterRequest[];
  oo_edges: PatternEdgeFilterRequest[];
}

export interface PatternEdgeFilterRequest {
  source: string;
  target: string;
}

export interface DfEdgeOption extends DfEdgeFilterRequest {
  label: string;
}

export type FilterDialogKind =
  | 'activities'
  | 'objectTypes'
  | 'dfNodes'
  | 'dfEdges'
  | 'timeframe'
  | 'textAttributes'
  | 'patterns';

export type GraphFilterMenu =
  | { kind: 'node'; activity: string; x: number; y: number }
  | { kind: 'edge'; source: string; target: string; x: number; y: number };

export interface AppliedFilterChip {
  kind: FilterDialogKind;
  label: string;
  description: string;
  removeLabel: string;
}
