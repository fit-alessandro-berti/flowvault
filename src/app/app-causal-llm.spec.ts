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

  it('asks the configured LLM to suggest a causal model', async () => {
    localStorage.setItem(
      'flowvault.llmConfig',
      JSON.stringify({
        provider: 'mistral',
        model: 'mistral-medium-3.5',
        apiKey: 'mk-test',
      }),
    );
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
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          choices: [
            {
              message: {
                content: JSON.stringify({
                  nodes: [
                    {
                      id: 'obs_create',
                      label: 'Order creation',
                      role: 'observable',
                      feature: 'activity.Create Order',
                      operation: 'log_10',
                    },
                    {
                      id: 'lat_flow',
                      label: 'Flow pressure',
                      role: 'latent',
                    },
                    {
                      id: 'out_close',
                      label: 'Closure',
                      role: 'outcome',
                      feature: 'activity.Close Order',
                      operation: 'identity',
                    },
                  ],
                  edges: [
                    { source: 'obs_create', target: 'lat_flow' },
                    { source: 'lat_flow', target: 'out_close' },
                  ],
                }),
              },
            },
          ],
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    );

    component.documentHandle = {
      causalFeatureTableJson: () =>
        JSON.stringify({
          object_type: 'Order',
          object_count: 2,
          feature_count: stateDetectionAnalysis.feature_count,
          feature_columns: stateDetectionAnalysis.feature_columns,
          table_preview: stateDetectionAnalysis.table_preview,
        }),
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

    Array.from(native.querySelectorAll<HTMLButtonElement>('.causal-controls button'))
      .find((button) => button.textContent?.includes('Ask LLM'))
      ?.click();
    await new Promise((resolve) => setTimeout(resolve, 0));
    fixture.detectChanges();

    const [, init] = fetchSpy.mock.calls[0];
    const body = JSON.parse(String((init as RequestInit).body));
    expect(fetchSpy.mock.calls[0][0]).toBe('https://api.mistral.ai/v1/chat/completions');
    expect(body.model).toBe('mistral-medium-3.5');
    expect(body.messages[1].content).toContain('Causal model JSON schema');
    expect(body.messages[1].content).toContain('activity.Create Order');
    expect(native.textContent).toContain('LLM suggested 3 nodes and 2 DAG edges');
    expect(native.textContent).toContain('Order creation');
    expect(native.textContent).toContain('Flow pressure');
    expect(native.textContent).toContain('Closure');
    expect(native.querySelectorAll<HTMLInputElement>('.causal-edge-grid input:checked').length).toBe(
      2,
    );

    fetchSpy.mockRestore();
  });
});
