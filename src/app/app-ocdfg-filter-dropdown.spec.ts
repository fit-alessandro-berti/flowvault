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

  it('opens OC-DFG filter dropdowns from graph node and edge clicks', () => {
    const fixture = TestBed.createComponent(App);
    let filterRequest = '';
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      summary: { set(value: unknown): void };
      originalSummary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
      selectedEventTypes: { set(value: string[]): void };
      selectedObjectTypes: { set(value: string[]): void };
      traditionalOcdfg: { set(value: unknown): void };
      activeFeature: { set(value: string): void };
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
        return JSON.stringify({ ...importedSummary, events: 1, objects: 1 });
      },
      originalSummaryJson: () => JSON.stringify(importedSummary),
      filteredObjectCentricDirectlyFollowsGraphJson: () => JSON.stringify(edgeFilterGraph),
    };
    component.summary.set(importedSummary);
    component.originalSummary.set(importedSummary);
    component.filterOptions.set({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      text_attributes: [],
    });
    component.selectedEventTypes.set(['Create Order', 'Close Order']);
    component.selectedObjectTypes.set(['Order', 'Item']);
    component.traditionalOcdfg.set(edgeFilterGraph);
    component.activeFeature.set('ocdfg');
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    native
      .querySelector<SVGElement>('g.process-node-activity .process-node-shape')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: 130, clientY: 140 }));
    fixture.detectChanges();

    expect(native.textContent).toContain('Filter objects containing this activity');
    Array.from(native.querySelectorAll<HTMLButtonElement>('.graph-filter-dropdown button'))
      .find((button) => button.textContent?.includes('activity'))
      ?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      df_nodes: ['Create Order'],
    });

    native.querySelector<HTMLButtonElement>('.filter-count-button')?.click();
    fixture.detectChanges();
    native.querySelector<HTMLButtonElement>('.filter-chip-remove')?.click();
    fixture.detectChanges();
    native
      .querySelector<SVGElement>('.process-edge-hitbox.is-filterable')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: 210, clientY: 140 }));
    fixture.detectChanges();

    expect(native.textContent).toContain('Directly-follows edge');
    Array.from(native.querySelectorAll<HTMLButtonElement>('.graph-filter-dropdown button'))
      .find((button) => button.textContent?.includes('edge'))
      ?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      df_edges: [{ source: 'Create Order', target: 'Close Order' }],
    });
  });
});
