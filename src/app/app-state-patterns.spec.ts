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

  it('opens state preset dialog after import', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      fileName: { set(value: string): void };
      summary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
      selectedObjectTypes: { set(value: string[]): void };
      openStateDialog(): void;
    };

    component.documentHandle = {};
    component.fileName.set('order-management.json');
    component.summary.set(importedSummary);
    component.filterOptions.set({ event_types: [], object_types: ['orders', 'items'] });
    component.selectedObjectTypes.set(['orders', 'items']);
    component.openStateDialog();
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    expect(native.querySelector('[role="dialog"]')).toBeTruthy();
    expect(native.querySelector('.info-icon-button')?.getAttribute('aria-describedby')).toBe(
      'state-language-help',
    );
    expect(native.textContent).toContain('State Definition Language');
    expect(native.textContent).toContain("STATE state FOR LEADING OBJECT TYPE 'Order'");
    expect(native.textContent).toContain('event."Stock After"');
    expect(native.textContent).toContain('Fulfillment Stage');
    expect(native.textContent).toContain('Value and Weight');
    expect(native.textContent).toContain('Leading object type');
    expect((native.querySelector('.state-leading-control select') as HTMLSelectElement).value).toBe(
      'orders',
    );
    expect((native.querySelector('textarea') as HTMLTextAreaElement).value).toContain(
      "FOR LEADING OBJECT TYPE 'orders'",
    );
  });

  it('loads and renders pattern selectors after applying states', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      summary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
      selectedObjectTypes: { set(value: string[]): void };
      stateQueryDraft: { set(value: string): void };
      applyStateQuery(): void;
      openStateDialog(): void;
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
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    expect(localStorage.getItem('flowvault.stateExpression')).toContain(
      "STATE state FOR LEADING OBJECT TYPE 'Order'",
    );
    expect(native.textContent).toContain('State Patterns');
    expect(native.querySelector('.feature-sidebar')).toBeTruthy();
    const stateAwareButton = Array.from(native.querySelectorAll<HTMLButtonElement>('.feature-button'))
      .find((button) => button.textContent?.includes('State-Aware OC-DFG'));
    expect(stateAwareButton?.disabled).toBe(false);
    expect(native.querySelector('.feature-button.is-selected')?.textContent).toContain('Patterns');
    expect(native.textContent).toContain('5x | Open on Order');
    expect(native.textContent).not.toContain('3x | Open -> Closed on Order');
    expect(native.querySelectorAll('.pattern-select').length).toBe(1);
    expect(native.querySelector('.pattern-tab-button.is-selected')?.textContent).toContain(
      'Intra-State',
    );

    native.querySelectorAll<HTMLButtonElement>('.pattern-tab-button')[1].click();
    fixture.detectChanges();

    expect(native.textContent).toContain('3x | Open -> Closed on Order');
    expect(native.querySelectorAll('.pattern-select').length).toBe(1);
    expect(native.querySelector('.pattern-tab-button.is-selected')?.textContent).toContain(
      'Inter-State',
    );

    stateAwareButton?.click();
    fixture.detectChanges();

    expect(native.textContent).toContain('State-Aware Object-Centric Directly-Follows Graph');
    expect(native.querySelector('app-process-graph svg.process-graph')).toBeTruthy();
    expect(native.querySelector('app-process-graph path.process-edge')?.getAttribute('d')).toContain(
      'C',
    );
    expect(native.querySelector('app-process-graph path.process-edge')?.getAttribute('stroke')).toBe(
      'hsl(214 68% 38%)',
    );
    expect(native.querySelector('app-process-graph ellipse')?.getAttribute('stroke')).toBe(
      'hsl(214 68% 38%)',
    );
    expect(
      native.querySelector('app-process-graph marker')?.getAttribute('markerWidth'),
    ).toBe('4');

    component.openStateDialog();
    fixture.detectChanges();
    expect(native.textContent).toContain('Saved Expression');
    Array.from(native.querySelectorAll<HTMLButtonElement>('.preset-option'))
      .find((button) => button.textContent?.includes('Saved Expression'))
      ?.click();
    fixture.detectChanges();
    expect((native.querySelector('textarea[aria-label="SQL-like state query"]') as HTMLTextAreaElement).value).toContain(
      "STATE state FOR LEADING OBJECT TYPE 'Order'",
    );
  });
});
