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

  it('applies global OC-DFG, text attribute, and pattern filters', () => {
    const fixture = TestBed.createComponent(App);
    let filterRequest = '';
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      summary: { set(value: unknown): void };
      originalSummary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
      selectedEventTypes: { set(value: string[]): void };
      selectedObjectTypes: { set(value: string[]): void };
      patternAnalysis: { set(value: unknown): void };
      traditionalOcdfg: { set(value: unknown): void };
    };
    const edgeFilterGraph: ProcessGraph = {
      ...traditionalProcessGraph,
      nodes: [
        {
          ...traditionalProcessGraph.nodes[1],
          id: 'a',
          label: 'Create Order',
          kind: 'activity',
          lines: ['Create Order'],
        },
        {
          ...traditionalProcessGraph.nodes[1],
          id: 'b',
          label: 'Close Order',
          kind: 'activity',
          lines: ['Close Order'],
        },
      ],
      edges: [
        {
          ...traditionalProcessGraph.edges[0],
          id: 'ab',
          source: 'a',
          target: 'b',
        },
      ],
    };

    component.documentHandle = {
      applyFilter: (request: string) => {
        filterRequest = request;
        return JSON.stringify({ ...statefulSummary, events: 1, objects: 1 });
      },
      originalSummaryJson: () => JSON.stringify(statefulSummary),
      statePatternsJson: () => JSON.stringify(patternAnalysis),
      filteredStateAwareObjectCentricDirectlyFollowsGraphJson: () => JSON.stringify(processGraph),
      filteredObjectCentricDirectlyFollowsGraphJson: () => JSON.stringify(edgeFilterGraph),
    };
    component.summary.set(statefulSummary);
    component.originalSummary.set(statefulSummary);
    component.filterOptions.set({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      text_attributes: [{ scope: 'event', name: 'state', values: ['Open', 'Closed'] }],
    });
    component.selectedEventTypes.set(['Create Order', 'Close Order']);
    component.selectedObjectTypes.set(['Order', 'Item']);
    component.patternAnalysis.set(patternAnalysis);
    component.traditionalOcdfg.set(edgeFilterGraph);
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-button'))
      .find((button) => button.textContent?.includes('Filter'))
      ?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.filter-menu button'))
      .find((button) => button.textContent?.includes('OC-DFG Nodes'))
      ?.click();
    fixture.detectChanges();
    native.querySelector<HTMLInputElement>('.filter-modal .filter-options input')?.click();
    fixture.detectChanges();
    native.querySelector<HTMLButtonElement>('.state-modal-footer button:last-child')?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      df_nodes: ['Create Order'],
    });
    native.querySelector<HTMLButtonElement>('.filter-count-button')?.click();
    fixture.detectChanges();
    expect(native.textContent).toContain('OC-DFG nodes 1');

    native.querySelector<HTMLButtonElement>('.filter-chip-remove')?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-button'))
      .find((button) => button.textContent?.includes('Filter'))
      ?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.filter-menu button'))
      .find((button) => button.textContent?.includes('OC-DFG Edges'))
      ?.click();
    fixture.detectChanges();
    native.querySelector<HTMLInputElement>('.filter-modal .filter-options input')?.click();
    fixture.detectChanges();
    native.querySelector<HTMLButtonElement>('.state-modal-footer button:last-child')?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      df_edges: [{ source: 'Create Order', target: 'Close Order' }],
    });

    native.querySelector<HTMLButtonElement>('.filter-count-button')?.click();
    fixture.detectChanges();
    native.querySelector<HTMLButtonElement>('.filter-chip-remove')?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-button'))
      .find((button) => button.textContent?.includes('Filter'))
      ?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.filter-menu button'))
      .find((button) => button.textContent?.includes('Text Attributes'))
      ?.click();
    fixture.detectChanges();
    native.querySelector<HTMLInputElement>('.filter-modal .filter-options input')?.click();
    fixture.detectChanges();
    native.querySelector<HTMLButtonElement>('.state-modal-footer button:last-child')?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      text_attributes: [{ scope: 'event', name: 'state', values: ['Open'] }],
    });
    native.querySelector<HTMLButtonElement>('.filter-count-button')?.click();
    fixture.detectChanges();
    expect(native.textContent).toContain('state 1');

    native.querySelector<HTMLButtonElement>('.filter-chip-remove')?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.feature-button'))
      .find((button) => button.textContent?.includes('Patterns'))
      ?.click();
    fixture.detectChanges();
    native
      .querySelector<HTMLButtonElement>('.pattern-panel .graph-toolbar .ghost-button')
      ?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest).patterns).toEqual([
      {
        family: 'intra',
        leading_object_type: 'Order',
        state: 'Open',
        sequence: ['START Open', 'Create Order [Open]', 'END Open'],
        eo_edges: [{ source: 'Create Order [Open]', target: 'Item' }],
        oo_edges: [{ source: 'Order', target: 'Item' }],
      },
    ]);
    native.querySelector<HTMLButtonElement>('.filter-count-button')?.click();
    fixture.detectChanges();
    expect(native.textContent).toContain('Patterns 1');
  });
});
