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
  exportJson(): string;
  exportXml(): string;
  objectLifecycleJson(objectId: string): string;
  applyStateQuery(query: string): string;
  statePatternsJson(): string;
  free(): void;
}

export interface StateQueryResult {
  attribute: string;
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

export interface ImportedOcelDocument {
  document: OcelDocumentHandle;
  summary: OcelSummary;
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

    return { document, summary };
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
