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
  directlyFollowsGraphJson(objectType: string): string;
  objectCentricDirectlyFollowsGraphJson(): string;
  stateAwareObjectCentricDirectlyFollowsGraphJson(): string;
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
  OcelDocument: new (input: string, formatHint?: string) => OcelDocumentHandle;
}

@Injectable({
  providedIn: 'root',
})
export class OcelWasmService {
  private modulePromise?: Promise<OcelWasmModule>;

  async importDocument(input: string, formatHint?: string): Promise<ImportedOcelDocument> {
    const wasm = await this.loadModule();
    const document = new wasm.OcelDocument(input, formatHint);
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
