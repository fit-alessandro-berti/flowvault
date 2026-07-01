import { TestBed } from '@angular/core/testing';
import { vi } from 'vitest';
import { App } from './app';
import type { ProcessGraph } from './ocel-wasm.service';
import { importedSummary, patternAnalysis, processGraph, statefulSummary, traditionalProcessGraph } from './testing/core-fixtures';
import { lifecycleDetail, objectSearchResult, stateCorrelationAnalysis, transitionKpisAnalysis } from './testing/dashboard-fixtures';
import { stateDetectionAnalysis, stateDetectionCellDetail } from './testing/state-detection-fixtures';

describe('App', () => {
  beforeEach(async () => {
    localStorage.clear();
    await TestBed.configureTestingModule({
      imports: [App],
    }).compileComponents();
  });

  it('creates the inspector shell', () => {
    const fixture = TestBed.createComponent(App);
    fixture.detectChanges();

    expect(fixture.componentInstance).toBeTruthy();
    const native = fixture.nativeElement as HTMLElement;
    expect(native.querySelector('.toolbar-left strong')?.textContent).toContain('FLOWVAULT');
    expect(native.querySelector('.drop-title')?.textContent).toContain(
      'Drop an OCEL 2.0 JSON/XML file',
    );
  });

  it('keeps document actions hidden before import', () => {
    const fixture = TestBed.createComponent(App);
    fixture.detectChanges();

    const buttons = Array.from(
      (fixture.nativeElement as HTMLElement).querySelectorAll<HTMLButtonElement>(
        '.toolbar-actions button',
      ),
    );
    expect(buttons.map((button) => button.textContent?.trim())).toEqual(['LLM Config']);
    expect((fixture.nativeElement as HTMLElement).textContent).not.toContain('Export JSON');
  });

  it('exports JSON and XML from a single export menu', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      fileName: { set(value: string): void };
      summary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
    };
    const createObjectUrl = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:export');
    const revokeObjectUrl = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined);
    const clickSpy = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => undefined);

    component.documentHandle = {
      exportJson: () => '{"events":[]}',
      exportXml: () => '<log />',
    };
    component.fileName.set('orders.json');
    component.summary.set(importedSummary);
    component.filterOptions.set({ event_types: [], object_types: [] });
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    const topbarButtons = Array.from(
      native.querySelectorAll<HTMLButtonElement>('.toolbar-actions > button, .toolbar-actions > .toolbar-popover-anchor > button'),
    ).map((button) => button.textContent?.trim());
    expect(topbarButtons).toContain('Export');
    expect(topbarButtons).not.toContain('Export JSON');
    expect(topbarButtons).not.toContain('Export XML');
    expect(native.querySelector('.toolbar-actions input[type="file"]')).toBeFalsy();

    Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-actions button'))
      .find((button) => button.textContent?.trim() === 'Export')
      ?.click();
    fixture.detectChanges();

    expect(native.querySelectorAll('.export-menu button').length).toBe(2);
    expect(native.textContent).toContain('JSON');
    expect(native.textContent).toContain('XML');

    native.querySelector<HTMLButtonElement>('.export-menu button')?.click();
    fixture.detectChanges();
    expect(clickSpy).toHaveBeenCalledTimes(1);
    expect(createObjectUrl).toHaveBeenCalledTimes(1);
    expect(native.querySelector('.export-menu')).toBeFalsy();

    Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-actions button'))
      .find((button) => button.textContent?.trim() === 'Export')
      ?.click();
    fixture.detectChanges();
    native.querySelectorAll<HTMLButtonElement>('.export-menu button')[1]?.click();
    fixture.detectChanges();

    expect(clickSpy).toHaveBeenCalledTimes(2);
    expect(createObjectUrl).toHaveBeenCalledTimes(2);

    createObjectUrl.mockRestore();
    revokeObjectUrl.mockRestore();
    clickSpy.mockRestore();
  });

  it('shows only the upload area before import', () => {
    const fixture = TestBed.createComponent(App);
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;

    expect(native.querySelector('.upload-page')).toBeTruthy();
    expect(native.querySelectorAll('.summary-card').length).toBe(0);
    expect(native.querySelector('.feature-sidebar')).toBeFalsy();
    expect(native.querySelectorAll('.sample-button').length).toBe(8);
    expect(native.textContent).toContain('Or start with a sample');
  });

  it('imports bundled compressed samples from static assets', async () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      ocelWasm: {
        importDocument(input: ArrayBuffer, formatHint?: string): Promise<unknown>;
      };
    };
    let requestedUrl = '';
    let importedHint: string | undefined;
    let importedByteLength = 0;
    const previousFetch = globalThis.fetch;

    globalThis.fetch = (async (input: RequestInfo | URL) => {
      requestedUrl = input.toString();
      return {
        ok: true,
        arrayBuffer: async () => new Uint8Array([0x1f, 0x8b]).buffer,
      } as Response;
    }) as typeof fetch;
    component.ocelWasm = {
      importDocument: async (input: ArrayBuffer, formatHint?: string) => {
        importedHint = formatHint;
        importedByteLength = input.byteLength;
        return {
          document: {
            filteredObjectCentricDirectlyFollowsGraphJson: () =>
              JSON.stringify(traditionalProcessGraph),
            free: () => undefined,
          },
          summary: importedSummary,
          originalSummary: importedSummary,
          filterOptions: {
            event_types: ['Create Order'],
            object_types: ['Order'],
          },
        };
      },
    };

    try {
      fixture.detectChanges();
      (fixture.nativeElement as HTMLElement).querySelector<HTMLButtonElement>('.sample-button')?.click();
      await fixture.whenStable();
      fixture.detectChanges();
    } finally {
      globalThis.fetch = previousFetch;
    }

    const native = fixture.nativeElement as HTMLElement;
    expect(requestedUrl).toContain('/static/ocel2_compressed/ocel20_example.json.gz');
    expect(importedHint).toBe('json');
    expect(importedByteLength).toBe(2);
    expect(native.textContent).toContain('ocel20_example.json.gz');
    expect(native.textContent).toContain('Statistics');
  });
});
