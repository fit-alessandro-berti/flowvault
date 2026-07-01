import { Injectable } from '@angular/core';
import type { ImportedOcelDocument, OcelDocumentHandle, OcelSummary } from './wasm-models/document.models';
import type { OcelFilterOptions } from './wasm-models/filter-options.models';

export * from './wasm-models/causal.models';
export * from './wasm-models/correlation.models';
export * from './wasm-models/document.models';
export * from './wasm-models/filter-options.models';
export * from './wasm-models/lifecycle.models';
export * from './wasm-models/process-graph.models';
export * from './wasm-models/state-detection.models';
export * from './wasm-models/state-pattern.models';
export * from './wasm-models/time-perspective.models';
export * from './wasm-models/transition-kpi.models';

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
