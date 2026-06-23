import { Injectable } from '@angular/core';

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
  statePatternsJson(): string;
  stateDetectionJson(requestJson: string): string;
  stateDetectionCellJson(requestJson: string): string;
  stateFeatureTableCsv(requestJson: string): string;
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

export interface StatePatternEdge {
  source: string;
  target: string;
  weight: number;
}

export interface StatePattern {
  id: string;
  family: 'intra' | 'inter';
  label: string;
  leading_object_type: string;
  state?: string;
  from_state?: string;
  to_state?: string;
  support: number;
  mass: number;
  sequence: string[];
  object_types: string[];
  df_edges: StatePatternEdge[];
  eo_edges: StatePatternEdge[];
  oo_edges: StatePatternEdge[];
}

export interface StatePatternAnalysis {
  intra: StatePattern[];
  inter: StatePattern[];
}

export interface StateDetectionResult {
  object_type: string;
  window_size: number;
  som_width: number;
  som_height: number;
  object_count: number;
  feature_count: number;
  window_count: number;
  color_attribute: string;
  color_attributes: StateDetectionColorOption[];
  feature_columns: string[];
  table_preview: StateDetectionPreviewRow[];
  pca: StateDetectionPca;
  som: StateDetectionSom;
  windows: StateDetectionWindow[];
}

export interface StateDetectionColorOption {
  id: string;
  label: string;
  kind: 'count' | 'numeric' | 'categorical';
}

export interface StateDetectionPreviewRow {
  object_id: string;
  values: number[];
}

export interface StateDetectionPca {
  pc1_variance: number;
  pc2_variance: number;
  pc1_explained_ratio: number;
  pc2_explained_ratio: number;
}

export interface StateDetectionSom {
  cells: StateDetectionSomCell[];
  transitions: StateDetectionSomTransition[];
}

export interface StateDetectionSomCell {
  x: number;
  y: number;
  label: string;
  count: number;
  color_value: number;
  color_label: string;
  color_kind: string;
  avg_pc1: number;
  avg_pc2: number;
  dominant_activity?: string;
}

export interface StateDetectionSomTransition {
  source_x: number;
  source_y: number;
  target_x: number;
  target_y: number;
  count: number;
  distance: number;
  nearby: boolean;
}

export interface StateDetectionWindow {
  object_id: string;
  start_event: string;
  end_event: string;
  pc1: number;
  pc2: number;
  cell_x: number;
  cell_y: number;
}

export interface StateDetectionCellDetail {
  cell: StateDetectionSomCell;
  dfg: ProcessGraph;
  entering_windows: StateDetectionBoundaryWindow[];
  exiting_windows: StateDetectionBoundaryWindow[];
}

export interface StateDetectionBoundaryWindow {
  object_id: string;
  start_event: string;
  end_event: string;
  source_cell: string;
  target_cell: string;
  pc1: number;
  pc2: number;
  activities: string[];
}

export interface ProcessGraphPoint {
  x: number;
  y: number;
}

export interface ProcessGraphNode {
  id: string;
  label: string;
  kind: string;
  shape: 'rect' | 'ellipse';
  color: string;
  object_type?: string;
  count: number;
  x: number;
  y: number;
  width: number;
  height: number;
  lines: string[];
}

export interface ProcessGraphEdge {
  id: string;
  source: string;
  target: string;
  kind: string;
  path: string;
  label: string;
  title: string;
  weight: number;
  object_type: string;
  color: string;
  directed: boolean;
  points: ProcessGraphPoint[];
  label_x: number;
  label_y: number;
  object_types: Array<{ object_type: string; weight: number }>;
}

export interface ProcessGraph {
  title: string;
  subtitle: string;
  width: number;
  height: number;
  nodes: ProcessGraphNode[];
  edges: ProcessGraphEdge[];
}

export interface ProcessGraphSettings {
  object_types: string[];
  min_activity_frequency: number;
  min_path_frequency: number;
}

export interface ImportedOcelDocument {
  document: OcelDocumentHandle;
  summary: OcelSummary;
  originalSummary: OcelSummary;
  filterOptions: OcelFilterOptions;
}

export interface OcelFilterOptions {
  event_types: string[];
  object_types: string[];
}

interface OcelWasmModule {
  default(options?: {
    module_or_path?: string | URL | Request | Response | BufferSource | WebAssembly.Module;
  }): Promise<unknown>;
  OcelDocument: {
    new (input: string, formatHint?: string): OcelDocumentHandle;
    fromBytes(input: Uint8Array, formatHint?: string): OcelDocumentHandle;
  };
}

@Injectable({
  providedIn: 'root',
})
export class OcelWasmService {
  private modulePromise?: Promise<OcelWasmModule>;

  async importDocument(
    input: string | ArrayBuffer | Uint8Array,
    formatHint?: string,
  ): Promise<ImportedOcelDocument> {
    const wasm = await this.loadModule();
    const document =
      typeof input === 'string'
        ? new wasm.OcelDocument(input, formatHint)
        : wasm.OcelDocument.fromBytes(
            input instanceof Uint8Array ? input : new Uint8Array(input),
            formatHint,
          );
    const summary = JSON.parse(document.summaryJson()) as OcelSummary;
    const originalSummary = JSON.parse(document.originalSummaryJson()) as OcelSummary;
    const filterOptions = JSON.parse(document.filterOptionsJson()) as OcelFilterOptions;

    return { document, summary, originalSummary, filterOptions };
  }

  private loadModule(): Promise<OcelWasmModule> {
    this.modulePromise ??= this.initializeModule();
    return this.modulePromise;
  }

  private async initializeModule(): Promise<OcelWasmModule> {
    const wasmDirectory = new URL('wasm/', document.baseURI);
    const moduleUrl = new URL('ocel_wasm.js', wasmDirectory);
    const wasmUrl = new URL('ocel_wasm_bg.wasm', wasmDirectory);
    const wasm = (await import(/* @vite-ignore */ moduleUrl.href)) as OcelWasmModule;

    await wasm.default({ module_or_path: wasmUrl });
    return wasm;
  }
}
