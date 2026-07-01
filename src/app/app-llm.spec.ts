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

  it('saves and tests LLM configuration', async () => {
    const fixture = TestBed.createComponent(App);
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          choices: [{ message: { content: 'OK' } }],
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    );
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-button'))
      .find((button) => button.textContent?.includes('LLM Config'))
      ?.click();
    fixture.detectChanges();

    const providerSelect = native.querySelector<HTMLSelectElement>('.llm-config-body select');
    const modelInput = native.querySelector<HTMLInputElement>(
      '.llm-config-body input[type="text"]',
    );
    const apiKeyInput = native.querySelector<HTMLInputElement>(
      '.llm-config-body input[type="password"]',
    );
    if (!providerSelect || !modelInput || !apiKeyInput) {
      throw new Error('missing LLM config controls');
    }

    providerSelect.value = 'openrouter';
    providerSelect.dispatchEvent(new Event('change'));
    fixture.detectChanges();
    expect(modelInput.value).toBe('openai/gpt-5.4');

    apiKeyInput.value = 'sk-or-test';
    apiKeyInput.dispatchEvent(new Event('input'));
    fixture.detectChanges();

    native.querySelector<HTMLButtonElement>('.state-modal-footer .ghost-button')?.click();
    await new Promise((resolve) => setTimeout(resolve, 0));
    fixture.detectChanges();

    expect(fetchSpy).toHaveBeenCalledWith(
      'https://openrouter.ai/api/v1/chat/completions',
      expect.objectContaining({
        method: 'POST',
      }),
    );
    expect(native.textContent).toContain('Test succeeded');

    native.querySelector<HTMLButtonElement>('.state-modal-footer button:last-child')?.click();
    const stored = JSON.parse(localStorage.getItem('flowvault.llmConfig') ?? '{}');
    expect(stored).toEqual({
      provider: 'openrouter',
      model: 'openai/gpt-5.4',
      apiKey: 'sk-or-test',
    });

    fetchSpy.mockRestore();
  });

  it('asks the configured LLM for a state expression', async () => {
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
      openStateDialog(): void;
    };
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          choices: [
            {
              message: {
                content: `\`\`\`sql
STATE state FOR LEADING OBJECT TYPE 'Order' AS CASE
  WHEN event.type = 'Close Order' THEN 'Closed'
  ELSE 'Open'
END
\`\`\``,
              },
            },
          ],
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      ),
    );

    component.documentHandle = {};
    component.fileName.set('orders.json');
    component.summary.set(importedSummary);
    component.originalSummary.set(importedSummary);
    component.filterOptions.set({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
    });
    component.selectedObjectTypes.set(['Order', 'Item']);
    component.selectedEventTypes.set(['Create Order', 'Close Order']);
    component.openStateDialog();
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    Array.from(native.querySelectorAll<HTMLButtonElement>('.preset-option'))
      .find((button) => button.textContent?.includes('Ask LLM'))
      ?.click();
    fixture.detectChanges();

    expect(native.textContent).toContain('LLM request');
    native.querySelector<HTMLButtonElement>('.llm-state-panel .ghost-button')?.click();
    await new Promise((resolve) => setTimeout(resolve, 0));
    fixture.detectChanges();

    const [, init] = fetchSpy.mock.calls[0];
    const body = JSON.parse(String((init as RequestInit).body));
    expect(fetchSpy.mock.calls[0][0]).toBe('https://api.mistral.ai/v1/chat/completions');
    expect(body.model).toBe('mistral-medium-3.5');
    expect(body.messages[1].content).toContain('Basic OCEL metadata');
    expect(body.messages[1].content).toContain('State expression language');
    expect(
      (native.querySelector('textarea[aria-label="SQL-like state query"]') as HTMLTextAreaElement)
        .value,
    ).toContain("WHEN event.type = 'Close Order' THEN 'Closed'");

    fetchSpy.mockRestore();
  });
});
