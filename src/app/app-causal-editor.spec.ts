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

  it('builds and fits a causal model from feature table variables', () => {
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
    let featureRequest = '';
    let fitRequest = '';

    component.documentHandle = {
      causalFeatureTableJson: (request: string) => {
        featureRequest = request;
        return JSON.stringify({
          object_type: 'Order',
          object_count: 2,
          feature_count: stateDetectionAnalysis.feature_count,
          feature_columns: stateDetectionAnalysis.feature_columns,
          table_preview: stateDetectionAnalysis.table_preview,
        });
      },
      fitCausalModelJson: (request: string) => {
        fitRequest = request;
        return JSON.stringify({
          object_type: 'Order',
          sample_count: 2,
          nodes: [
            {
              id: 'obs-1',
              label: 'Create Order',
              role: 'observable',
              feature: 'activity.Create Order',
              operation: 'log10',
              mean: 0.1,
              std_dev: 0.2,
            },
            {
              id: 'lat-1',
              label: 'Process intensity',
              role: 'latent',
              operation: 'identity',
              mean: 0,
              std_dev: 1,
            },
            {
              id: 'out-1',
              label: 'Close Order',
              role: 'outcome',
              feature: 'activity.Close Order',
              operation: 'identity',
              mean: 0.5,
              std_dev: 0.5,
            },
          ],
          edges: [
            {
              source: 'obs-1',
              target: 'lat-1',
              correlation: 0.75,
              intensity: 0.75,
              p_value: 0.2,
              sample_count: 2,
            },
            {
              source: 'lat-1',
              target: 'out-1',
              correlation: 0.5,
              intensity: 0.5,
              p_value: 0.4,
              sample_count: 2,
            },
          ],
        });
      },
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
      .find((button) => button.textContent?.includes('Causal Model'))
      ?.click();
    fixture.detectChanges();

    expect(JSON.parse(featureRequest)).toEqual({ object_type: 'Order' });
    expect(native.textContent).toContain('Features');
    expect(native.textContent).toContain('activity.Create Order');

    const featureRows = native.querySelectorAll<HTMLElement>('.causal-feature-row');
    featureRows[0].querySelectorAll<HTMLButtonElement>('.mini-button')[0]?.click();
    fixture.detectChanges();
    featureRows[1].querySelectorAll<HTMLButtonElement>('.mini-button')[1]?.click();
    fixture.detectChanges();

    const operationSelect = native.querySelectorAll<HTMLSelectElement>('.causal-node-card select')[1];
    operationSelect.value = 'log10';
    operationSelect.dispatchEvent(new Event('change'));
    fixture.detectChanges();

    const latentInput = native.querySelector<HTMLInputElement>('.causal-latent-add input');
    if (latentInput) {
      latentInput.value = 'Process intensity';
      latentInput.dispatchEvent(new Event('input'));
    }
    native.querySelector<HTMLButtonElement>('.causal-latent-add .mini-button')?.click();
    fixture.detectChanges();

    const checkboxes = native.querySelectorAll<HTMLInputElement>('.causal-edge-grid input[type="checkbox"]');
    checkboxes[0].click();
    fixture.detectChanges();
    checkboxes[1].click();
    fixture.detectChanges();

    native.querySelector<HTMLButtonElement>('.causal-controls button:last-child')?.click();
    fixture.detectChanges();

    const request = JSON.parse(fitRequest);
    expect(request.object_type).toBe('Order');
    expect(request.nodes).toEqual([
      {
        id: 'obs-1',
        label: 'Create Order',
        role: 'observable',
        feature: 'activity.Create Order',
        operation: 'log10',
      },
      {
        id: 'out-1',
        label: 'Close Order',
        role: 'outcome',
        feature: 'activity.Close Order',
        operation: 'identity',
      },
      {
        id: 'lat-1',
        label: 'Process intensity',
        role: 'latent',
        operation: 'identity',
      },
    ]);
    expect(request.edges).toEqual([
      { source: 'obs-1', target: 'lat-1' },
      { source: 'lat-1', target: 'out-1' },
    ]);
    expect(native.textContent).toContain('Fitted Graph');
    expect(native.textContent).toContain('I=0.75 / p=0.2');
  });
});
