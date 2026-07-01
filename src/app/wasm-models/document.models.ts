import type { OcelFilterOptions } from './filter-options.models';

export interface OcelSummary {
  source_format: 'json' | 'xml';
  event_types: number;
  object_types: number;
  events: number;
  objects: number;
  e2o_relationships: number;
  o2o_relationships: number;
  interned_strings: number;
  objects_with_lifecycle: number;
  stateful_events: number;
}

export interface OcelDocumentHandle {
  summaryJson(): string;
  originalSummaryJson(): string;
  filterOptionsJson(): string;
  applyFilter(filterJson: string): string;
  exportJson(): string;
  exportXml(): string;
  objectLifecycleJson(objectId: string): string;
  applyStateQuery(query: string): string;
  applyStateDetection(requestJson: string): string;
  statePatternsJson(): string;
  stateDetectionJson(requestJson: string): string;
  stateDetectionCellJson(requestJson: string): string;
  stateFeatureTableCsv(requestJson: string): string;
  stateCorrelationsJson(): string;
  timePerspectiveJson(requestJson: string): string;
  stateTransitionKpisJson(requestJson: string): string;
  objectSearchJson(requestJson: string): string;
  objectLifecycleDetailJson(objectId: string): string;
  causalFeatureTableJson(requestJson: string): string;
  causalFeatureTableCsv(requestJson: string): string;
  fitCausalModelJson(requestJson: string): string;
  directlyFollowsGraphJson(objectType: string): string;
  objectCentricDirectlyFollowsGraphJson(): string;
  filteredObjectCentricDirectlyFollowsGraphJson(requestJson: string): string;
  stateAwareObjectCentricDirectlyFollowsGraphJson(): string;
  filteredStateAwareObjectCentricDirectlyFollowsGraphJson(requestJson: string): string;
  free(): void;
}

export interface StateQueryResult {
  attribute: string;
  leading_object_type: string;
  assigned_events: number;
  total_events: number;
}

export interface ImportedOcelDocument {
  document: OcelDocumentHandle;
  summary: OcelSummary;
  originalSummary: OcelSummary;
  filterOptions: OcelFilterOptions;
}
