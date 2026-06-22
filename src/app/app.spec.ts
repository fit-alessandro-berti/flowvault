import { TestBed } from '@angular/core/testing';
import { App } from './app';
import { StatePatternAnalysis } from './ocel-wasm.service';

const importedSummary = {
  source_format: 'json' as const,
  event_types: 1,
  object_types: 2,
  events: 2,
  objects: 2,
  e2o_relationships: 3,
  o2o_relationships: 1,
  interned_strings: 8,
  objects_with_lifecycle: 2,
  stateful_events: 0,
};

const statefulSummary = {
  ...importedSummary,
  stateful_events: 2,
};

const patternAnalysis: StatePatternAnalysis = {
  intra: [
    {
      id: 'intra-1',
      family: 'intra',
      label: 'Open on Order',
      leading_object_type: 'Order',
      state: 'Open',
      support: 5,
      mass: 10,
      sequence: ['START Open', 'Create Order [Open]', 'END Open'],
      object_types: ['Order', 'Item'],
      df_edges: [
        { source: 'START Open', target: 'Create Order [Open]', weight: 5 },
        { source: 'Create Order [Open]', target: 'END Open', weight: 5 },
      ],
      eo_edges: [{ source: 'Create Order [Open]', target: 'Item', weight: 5 }],
      oo_edges: [{ source: 'Order', target: 'Item', weight: 5 }],
    },
  ],
  inter: [
    {
      id: 'inter-1',
      family: 'inter',
      label: 'Open -> Closed on Order',
      leading_object_type: 'Order',
      from_state: 'Open',
      to_state: 'Closed',
      support: 3,
      mass: 9,
      sequence: [
        'START Open',
        'Create Order [Open]',
        'CHANGE Open -> Closed',
        'Close Order [Closed]',
        'END Closed',
      ],
      object_types: ['Order', 'Item'],
      df_edges: [
        { source: 'START Open', target: 'Create Order [Open]', weight: 3 },
        { source: 'Create Order [Open]', target: 'CHANGE Open -> Closed', weight: 3 },
        { source: 'CHANGE Open -> Closed', target: 'Close Order [Closed]', weight: 3 },
        { source: 'Close Order [Closed]', target: 'END Closed', weight: 3 },
      ],
      eo_edges: [{ source: 'Close Order [Closed]', target: 'Item', weight: 3 }],
      oo_edges: [{ source: 'Order', target: 'Item', weight: 3 }],
    },
  ],
};

describe('App', () => {
  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [App],
    }).compileComponents();
  });

  it('creates the inspector shell', () => {
    const fixture = TestBed.createComponent(App);
    fixture.detectChanges();

    expect(fixture.componentInstance).toBeTruthy();
    expect((fixture.nativeElement as HTMLElement).querySelector('h1')?.textContent).toContain(
      'OCEL 2.0 Inspector',
    );
  });

  it('keeps export buttons disabled before import', () => {
    const fixture = TestBed.createComponent(App);
    fixture.detectChanges();

    const buttons = Array.from((fixture.nativeElement as HTMLElement).querySelectorAll('button'));
    expect(buttons.length).toBe(3);
    expect(buttons.every((button) => button.disabled)).toBe(true);
  });

  it('renders empty summary counts initially', () => {
    const fixture = TestBed.createComponent(App);
    fixture.detectChanges();

    const values = Array.from(
      (fixture.nativeElement as HTMLElement).querySelectorAll('.summary-card strong'),
    ).map((element) => element.textContent?.trim());

    expect(values).toEqual(['0', '0', '0', '0']);
  });

  it('opens state preset dialog after import', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      fileName: { set(value: string): void };
      summary: { set(value: unknown): void };
      openStateDialog(): void;
    };

    component.documentHandle = {};
    component.fileName.set('order-management.json');
    component.summary.set(importedSummary);
    component.openStateDialog();
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    expect(native.querySelector('[role="dialog"]')).toBeTruthy();
    expect(native.textContent).toContain('Fulfillment Stage');
    expect(native.textContent).toContain('Value and Weight');
    expect((native.querySelector('textarea') as HTMLTextAreaElement).value).toContain(
      "event.type = 'failed delivery'",
    );
  });

  it('loads and renders pattern selectors after applying states', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      summary: { set(value: unknown): void };
      stateQueryDraft: { set(value: string): void };
      applyStateQuery(): void;
    };

    component.documentHandle = {
      applyStateQuery: () =>
        JSON.stringify({ attribute: 'state', assigned_events: 2, total_events: 2 }),
      summaryJson: () => JSON.stringify(statefulSummary),
      statePatternsJson: () => JSON.stringify(patternAnalysis),
    };
    component.summary.set(importedSummary);
    component.stateQueryDraft.set("STATE state AS CASE WHEN event.type = 'x' THEN 'Open' END");
    component.applyStateQuery();
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    expect(native.textContent).toContain('State Patterns');
    expect(native.textContent).toContain('5x | Open on Order');
    expect(native.textContent).toContain('3x | Open -> Closed on Order');
    expect(native.querySelectorAll('.pattern-select').length).toBe(2);
  });

  it('renders the graphical pattern view', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      summary: { set(value: unknown): void };
      stateQueryDraft: { set(value: string): void };
      intraVisualization: { set(value: string): void };
      interVisualization: { set(value: string): void };
      applyStateQuery(): void;
    };

    component.documentHandle = {
      applyStateQuery: () =>
        JSON.stringify({ attribute: 'state', assigned_events: 2, total_events: 2 }),
      summaryJson: () => JSON.stringify(statefulSummary),
      statePatternsJson: () => JSON.stringify(patternAnalysis),
    };
    component.summary.set(importedSummary);
    component.stateQueryDraft.set("STATE state AS CASE WHEN event.type = 'x' THEN 'Open' END");
    component.applyStateQuery();
    component.intraVisualization.set('graph');
    component.interVisualization.set('graph');
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    expect(native.querySelectorAll('svg.pattern-graph').length).toBe(2);
    expect(native.querySelectorAll('.graph-node-control').length).toBeGreaterThan(0);
    expect(native.querySelectorAll('.graph-edge-df').length).toBeGreaterThan(0);
    expect(native.querySelectorAll('.graph-node tspan').length).toBeGreaterThan(
      native.querySelectorAll('.graph-node').length,
    );
    expect(native.querySelector('.graph-edge-oo')?.getAttribute('marker-end')).toBeNull();
  });
});
