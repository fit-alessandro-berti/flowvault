import type { OcelSummary } from '../ocel-wasm.service';

export interface SummaryCard {
  label: string;
  value: SummaryDisplayValue;
}

export interface SummaryDisplayValue {
  current: string;
  original?: string;
  filtered: boolean;
}

export type SummaryMetric = keyof Pick<
  OcelSummary,
  | 'event_types'
  | 'object_types'
  | 'events'
  | 'objects'
  | 'e2o_relationships'
  | 'o2o_relationships'
  | 'interned_strings'
  | 'objects_with_lifecycle'
  | 'stateful_events'
>;
