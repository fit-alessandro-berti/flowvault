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

  it('renders state detection analysis and downloads the feature table', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      fileName: { set(value: string): void };
      summary: { set(value: unknown): void };
      originalSummary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
      selectedObjectTypes: { set(value: string[]): void };
      selectedEventTypes: { set(value: string[]): void };
    };
    let stateDetectionRequest = '';
    let stateDetectionCellRequest = '';
    let stateDetectionApplyRequest = '';
    let csvRequest = '';
    const createObjectUrl = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:features');
    const revokeObjectUrl = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined);
    const clickSpy = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => undefined);

    component.documentHandle = {
      stateDetectionJson: (request: string) => {
        stateDetectionRequest = request;
        return JSON.stringify(stateDetectionAnalysis);
      },
      stateDetectionCellJson: (request: string) => {
        stateDetectionCellRequest = request;
        return JSON.stringify(stateDetectionCellDetail);
      },
      stateFeatureTableCsv: (request: string) => {
        csvRequest = request;
        return 'object_id,activity.Create Order\nO1,1\n';
      },
      applyStateDetection: (request: string) => {
        stateDetectionApplyRequest = request;
        return JSON.stringify({
          attribute: 'state',
          leading_object_type: 'Order',
          assigned_events: 2,
          total_events: 2,
        });
      },
      filterOptionsJson: () =>
        JSON.stringify({
          event_types: ['Create Order', 'Close Order'],
          object_types: ['Order', 'Item'],
          text_attributes: [{ scope: 'event', name: 'state', values: ['S1-1', 'S1-2'] }],
        }),
      summaryJson: () => JSON.stringify(statefulSummary),
      originalSummaryJson: () => JSON.stringify(statefulSummary),
      statePatternsJson: () => JSON.stringify(patternAnalysis),
      stateAwareObjectCentricDirectlyFollowsGraphJson: () => JSON.stringify(processGraph),
      filteredStateAwareObjectCentricDirectlyFollowsGraphJson: () => JSON.stringify(processGraph),
    };
    component.fileName.set('orders.json');
    component.summary.set(importedSummary);
    component.originalSummary.set(importedSummary);
    component.filterOptions.set({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
    });
    component.selectedEventTypes.set(['Create Order', 'Close Order']);
    component.selectedObjectTypes.set(['Order', 'Item']);
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    Array.from(native.querySelectorAll<HTMLButtonElement>('.feature-button'))
      .find((button) => button.textContent?.includes('State Detection'))
      ?.click();
    fixture.detectChanges();

    expect(JSON.parse(stateDetectionRequest)).toEqual({
      object_type: 'Order',
      window_size: 4,
      som_width: 3,
      som_height: 3,
      color_attribute: '__window_count',
    });
    expect(native.textContent).toContain('State Detection');
    expect(native.textContent).toContain('Feature Table');
    expect(native.textContent).toContain('Self-Organizing Map');
    expect(native.textContent).toContain('State Transitions');
    expect(native.textContent).toContain('85%');
    expect(native.textContent).not.toContain('PCA Windows');
    expect(native.textContent).toContain('priority: High (2)');
    expect(native.querySelectorAll('.som-cell').length).toBe(4);
    expect(native.querySelector('.som-cell')?.getAttribute('title')).toContain('Create Order');
    expect(native.querySelector('.transition-list .is-nearby')).toBeTruthy();

    native.querySelector<HTMLButtonElement>('.feature-table-preview .ghost-button')?.click();

    expect(JSON.parse(csvRequest).object_type).toBe('Order');
    expect(createObjectUrl).toHaveBeenCalled();
    expect(clickSpy).toHaveBeenCalled();

    native.querySelector<HTMLButtonElement>('.som-cell')?.click();
    fixture.detectChanges();

    expect(JSON.parse(stateDetectionCellRequest)).toEqual({
      object_type: 'Order',
      window_size: 4,
      som_width: 3,
      som_height: 3,
      color_attribute: 'attribute::priority',
      cell_x: 0,
      cell_y: 0,
    });
    expect(native.querySelector('.state-detection-cell-modal')).toBeTruthy();
    expect(native.textContent).toContain('Entering windows');
    expect(native.textContent).toContain('Exiting windows');
    expect(native.textContent).toContain('Object-Centric Directly-Follows Graph');
    expect(native.querySelector('app-process-graph svg.process-graph')).toBeTruthy();

    native.querySelectorAll<HTMLButtonElement>('.state-detection-cell-tabs button')[1].click();
    fixture.detectChanges();
    expect(native.textContent).toContain('Entering Windows: S1-1');
    expect(native.querySelector('app-process-graph svg.process-graph')).toBeTruthy();

    native.querySelectorAll<HTMLButtonElement>('.state-detection-cell-tabs button')[2].click();
    fixture.detectChanges();
    expect(native.textContent).toContain('Exiting Windows: S1-1');
    expect(native.querySelector('app-process-graph svg.process-graph')).toBeTruthy();

    native.querySelector<HTMLButtonElement>('.state-detection-cell-modal .ghost-button')?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.state-detection-controls button'))
      .find((button) => button.textContent?.includes('Apply'))
      ?.click();
    fixture.detectChanges();

    expect(JSON.parse(stateDetectionApplyRequest)).toEqual({
      object_type: 'Order',
      window_size: 4,
      som_width: 3,
      som_height: 3,
      color_attribute: 'attribute::priority',
    });
    expect(native.textContent).toContain('Added state for Order from 3 x 3 SOM windows');

    createObjectUrl.mockRestore();
    revokeObjectUrl.mockRestore();
    clickSpy.mockRestore();
  });
});
