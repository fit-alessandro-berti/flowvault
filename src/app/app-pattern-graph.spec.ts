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

  it('renders the graphical pattern view', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      summary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
      selectedObjectTypes: { set(value: string[]): void };
      stateQueryDraft: { set(value: string): void };
      intraVisualization: { set(value: string): void };
      interVisualization: { set(value: string): void };
      applyStateQuery(): void;
    };

    component.documentHandle = {
      applyStateQuery: () =>
        JSON.stringify({
          attribute: 'state',
          leading_object_type: 'Order',
          assigned_events: 2,
          total_events: 2,
        }),
      filterOptionsJson: () =>
        JSON.stringify({
          event_types: [],
          object_types: ['Order'],
          text_attributes: [{ scope: 'event', name: 'state', values: ['Open', 'Closed'] }],
        }),
      summaryJson: () => JSON.stringify(statefulSummary),
      originalSummaryJson: () => JSON.stringify(statefulSummary),
      statePatternsJson: () => JSON.stringify(patternAnalysis),
      stateAwareObjectCentricDirectlyFollowsGraphJson: () => JSON.stringify(processGraph),
      filteredStateAwareObjectCentricDirectlyFollowsGraphJson: () => JSON.stringify(processGraph),
    };
    component.summary.set(importedSummary);
    component.filterOptions.set({ event_types: [], object_types: ['Order'] });
    component.selectedObjectTypes.set(['Order']);
    component.stateQueryDraft.set(
      "STATE state FOR LEADING OBJECT TYPE 'Order' AS CASE WHEN event.type = 'x' THEN 'Open' END",
    );
    component.applyStateQuery();
    component.intraVisualization.set('graph');
    component.interVisualization.set('graph');
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    expect(native.querySelectorAll('svg.pattern-graph').length).toBe(1);
    expect(native.querySelectorAll('.graph-node-control').length).toBeGreaterThan(0);
    expect(native.querySelectorAll('.graph-edge-df').length).toBeGreaterThan(0);
    expect(native.querySelectorAll('.graph-node tspan').length).toBeGreaterThan(
      native.querySelectorAll('.graph-node').length,
    );
    expect(native.querySelector('.graph-edge-oo')?.getAttribute('marker-end')).toBeNull();
    expect(native.querySelector('svg.pattern-graph marker')?.getAttribute('markerWidth')).toBe(
      '4',
    );

    native.querySelectorAll<HTMLButtonElement>('.pattern-tab-button')[1].click();
    fixture.detectChanges();

    expect(native.querySelectorAll('svg.pattern-graph').length).toBe(1);
    expect(native.querySelectorAll('.graph-node-change').length).toBeGreaterThan(0);

    const openButtons = native.querySelectorAll<HTMLButtonElement>('.graph-open-button');
    expect(openButtons.length).toBe(1);
    openButtons[0].click();
    fixture.detectChanges();

    expect(native.querySelector('.graph-modal')).toBeTruthy();
    expect(native.querySelector('svg.pattern-graph-expanded')).toBeTruthy();
    expect(
      native.querySelector('svg.pattern-graph-expanded marker')?.getAttribute('markerWidth'),
    ).toBe('5');
    expect(native.textContent).toContain('Open -> Closed on Order');

    native.querySelector<HTMLButtonElement>('.graph-modal .ghost-button')?.click();
    fixture.detectChanges();

    expect(native.querySelector('.graph-modal')).toBeFalsy();
  });
});
