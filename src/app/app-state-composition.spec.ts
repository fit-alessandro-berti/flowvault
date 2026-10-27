import { TestBed } from '@angular/core/testing';
import { vi } from 'vitest';
import { App } from './app';
import { withLeadingObjectTypeClause } from './helpers/common.helpers';

const template =
  "STATE state FOR LEADING OBJECT TYPE 'Order' AS CASE WHEN object.status IS NOT NULL THEN object.status END";

describe('Named state query templates', () => {
  beforeEach(async () => {
    localStorage.clear();
    vi.clearAllMocks();
    await TestBed.configureTestingModule({ imports: [App] }).compileComponents();
  });

  afterEach(() => vi.restoreAllMocks());

  it('keeps the new attribute name when reusing a saved query or preset', () => {
    const vm = TestBed.createComponent(App).componentInstance as any;
    vm.filterOptions.set({ event_types: [], object_types: ['Order'], text_attributes: [] });
    vm.selectedLeadingObjectType.set('Order');
    vm.stateDefinitions.set([
      { attribute: 'state1', leading_object_type: 'Order', query: template },
    ]);
    vm.addStateAttribute();
    vm.persistedStateExpression.set(template);
    vm.selectPersistedStateExpression();
    expect(vm.stateQueryDraft()).toContain('STATE state2 FOR');
    expect(vm.stateQueryDraft()).toContain('THEN object.status');
    vm.selectStatePreset({
      id: 'example',
      name: 'Example',
      leadingObjectType: 'Order',
      query: template,
    });
    expect(vm.stateQueryDraft()).toContain('STATE state2 FOR');
  });

  it('keeps the target attribute when the LLM returns a default state name', async () => {
    const vm = TestBed.createComponent(App).componentInstance as any;
    vm.documentHandle = {};
    vm.filterOptions.set({ event_types: [], object_types: ['Order'], text_attributes: [] });
    vm.selectedLeadingObjectType.set('Order');
    vm.stateQueryDraft.set(template.replace('STATE state ', 'STATE shipping_state '));
    vi.spyOn(vm, 'currentLlmConfig').mockReturnValue({
      provider: 'openai',
      model: 'test-model',
      apiKey: 'test-key',
    });
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ choices: [{ message: { content: template } }] }), {
        status: 200,
      }),
    );
    await vm.generateStateExpressionWithLlm();
    expect(vm.stateQueryDraft()).toContain('STATE shipping_state FOR');
    expect(vm.stateQueryDraft()).toContain('THEN object.status');
    expect(JSON.parse(String(fetchMock.mock.calls[0][1]?.body)).messages[1].content).toContain(
      'Shape: STATE shipping_state FOR',
    );
    expect(vm.errorMessage()).toBe('');
  });

  it('preserves malformed query input for validation instead of substituting a default', () => {
    const invalid = 'STATE 123broken AS CASE';
    expect(withLeadingObjectTypeClause(invalid, 'Order')).toBe(invalid);
  });
});
