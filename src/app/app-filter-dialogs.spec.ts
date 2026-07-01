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

  it('applies filters from dialogs, renders chips, and recomputes state patterns', () => {
    const fixture = TestBed.createComponent(App);
    const filteredSummary = {
      ...importedSummary,
      event_types: 1,
      object_types: 1,
      events: 1,
      objects: 1,
      e2o_relationships: 1,
      o2o_relationships: 0,
      stateful_events: 1,
    };
    let filterRequest = '';
    let patternCalls = 0;
    const traditionalGraphRequests: string[] = [];
    const stateAwareGraphRequests: string[] = [];
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      summary: { set(value: unknown): void };
      originalSummary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
      selectedEventTypes: { set(value: string[]): void };
      selectedObjectTypes: { set(value: string[]): void };
      patternAnalysis: { set(value: unknown): void };
      stateMessage: { set(value: string): void };
    };

    component.documentHandle = {
      applyFilter: (request: string) => {
        filterRequest = request;
        return JSON.stringify(filteredSummary);
      },
      originalSummaryJson: () => JSON.stringify(statefulSummary),
      statePatternsJson: () => {
        patternCalls += 1;
        return JSON.stringify(patternAnalysis);
      },
      stateAwareObjectCentricDirectlyFollowsGraphJson: () => JSON.stringify(processGraph),
      filteredStateAwareObjectCentricDirectlyFollowsGraphJson: (request: string) => {
        stateAwareGraphRequests.push(request);
        return JSON.stringify(processGraph);
      },
      filteredObjectCentricDirectlyFollowsGraphJson: (request: string) => {
        traditionalGraphRequests.push(request);
        return JSON.stringify(traditionalProcessGraph);
      },
    };
    component.summary.set(statefulSummary);
    component.originalSummary.set(statefulSummary);
    component.filterOptions.set({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
    });
    component.selectedEventTypes.set(['Create Order', 'Close Order']);
    component.selectedObjectTypes.set(['Order', 'Item']);
    component.patternAnalysis.set(patternAnalysis);
    component.stateMessage.set('Added state to 2 of 2 events.');
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    const filterButton = Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-button'))
      .find((button) => button.textContent?.includes('Filter'));
    filterButton?.click();
    fixture.detectChanges();

    native.querySelectorAll<HTMLButtonElement>('.filter-menu button')[0].click();
    fixture.detectChanges();

    const closeOrderCheckbox = Array.from(
      native.querySelectorAll<HTMLInputElement>('.filter-modal .filter-options input'),
    )[1];
    closeOrderCheckbox.click();
    fixture.detectChanges();
    native.querySelector<HTMLButtonElement>('.state-modal-footer button:last-child')?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order'],
      object_types: ['Order', 'Item'],
    });
    expect(native.textContent).toContain('1/2');
    native.querySelector<HTMLButtonElement>('.filter-count-button')?.click();
    fixture.detectChanges();
    expect(native.textContent).toContain('Activities 1/2');
    expect(native.querySelector('.filter-chip')?.getAttribute('title')).toContain('Create Order');
    expect(native.textContent).toContain('State retained on 1 of 1 active events.');
    expect(patternCalls).toBe(1);

    const featureButtons = Array.from(native.querySelectorAll<HTMLButtonElement>('.feature-button'));
    featureButtons.find((button) => button.textContent?.includes('Object-Centric DFG'))?.click();
    fixture.detectChanges();

    expect(native.querySelectorAll('app-process-graph').length).toBe(1);
    expect(native.textContent).toContain('Object-Centric Directly-Follows Graph');
    expect(native.textContent).toContain('Activity Frequency');
    expect(native.querySelector('.process-node-count')?.textContent?.trim()).toMatch(/\d+/);
    expect(traditionalGraphRequests.length).toBe(1);
    expect(stateAwareGraphRequests.length).toBe(1);

    featureButtons.find((button) => button.textContent?.includes('State-Aware OC-DFG'))?.click();
    fixture.detectChanges();

    const stateAwareGraph = native.querySelector('app-process-graph');
    expect(native.textContent).toContain('State-Aware Object-Centric Directly-Follows Graph');
    const activityFrequency = stateAwareGraph?.querySelector<HTMLInputElement>(
      '.frequency-control input[type="range"]',
    );
    if (!activityFrequency) {
      throw new Error('missing activity frequency slider');
    }
    activityFrequency.value = '2';
    activityFrequency.dispatchEvent(new Event('input'));
    fixture.detectChanges();

    expect(stateAwareGraphRequests.length).toBe(1);

    stateAwareGraph?.querySelector<HTMLButtonElement>('.apply-button')?.click();
    fixture.detectChanges();

    expect(stateAwareGraphRequests.length).toBe(2);
    expect(JSON.parse(stateAwareGraphRequests[stateAwareGraphRequests.length - 1])).toEqual({
      object_types: ['Order', 'Item'],
      min_activity_frequency: 2,
      min_path_frequency: 1,
    });

    native.querySelector<HTMLButtonElement>('.filter-chip-remove')?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
    });
  });
});
