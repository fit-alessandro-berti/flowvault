import { TestBed } from '@angular/core/testing';
import { vi } from 'vitest';
import { App } from './app';
import {
  ProcessGraph,
  StateDetectionCellDetail,
  StateDetectionResult,
  StatePatternAnalysis,
} from './ocel-wasm.service';

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

const processGraph: ProcessGraph = {
  title: 'State-Aware Object-Centric Directly-Follows Graph',
  subtitle: 'State-enriched lifecycles collated across object types',
  width: 520,
  height: 220,
  nodes: [
    {
      id: 'n0',
      label: 'START\nOrder',
      kind: 'object-start',
      shape: 'ellipse',
      color: 'hsl(214 68% 38%)',
      object_type: 'Order',
      count: 1,
      x: 20,
      y: 60,
      width: 140,
      height: 72,
      lines: ['START', 'Order'],
    },
    {
      id: 'n1',
      label: 'Create Order [Open]',
      kind: 'state-activity',
      shape: 'rect',
      color: '#42635c',
      count: 2,
      x: 40,
      y: 60,
      width: 180,
      height: 68,
      lines: ['Create Order', '[Open]'],
    },
    {
      id: 'n2',
      label: 'CHANGE Open -> Closed',
      kind: 'state-change',
      shape: 'rect',
      color: '#42635c',
      count: 1,
      x: 300,
      y: 60,
      width: 190,
      height: 68,
      lines: ['CHANGE Open', '-> Closed'],
    },
  ],
  edges: [
    {
      id: 'e1',
      source: 'n1',
      target: 'n2',
      kind: 'df',
      path: 'M 220 94 C 240 78 280 110 300 94',
      label: '2',
      title: 'Order: 2',
      weight: 2,
      object_type: 'Order',
      color: 'hsl(214 68% 38%)',
      directed: true,
      points: [
        { x: 220, y: 94 },
        { x: 260, y: 94 },
        { x: 300, y: 94 },
      ],
      label_x: 260,
      label_y: 86,
      object_types: [{ object_type: 'Order', weight: 2 }],
    },
  ],
};

const traditionalProcessGraph: ProcessGraph = {
  ...processGraph,
  title: 'Object-Centric Directly-Follows Graph',
  subtitle: 'Flattened over selected object types with typed lifecycle edges',
  nodes: [
    processGraph.nodes[0],
    {
      ...processGraph.nodes[1],
      label: 'Create Order',
      kind: 'activity',
      count: 5,
      lines: ['Create Order'],
    },
  ],
  edges: [
    {
      ...processGraph.edges[0],
      source: 'n0',
      target: 'n1',
      weight: 5,
      label: '5',
      title: 'Order: 5',
    },
  ],
};

const stateDetectionAnalysis: StateDetectionResult = {
  object_type: 'Order',
  window_size: 2,
  som_width: 2,
  som_height: 2,
  color_attribute: 'attribute::priority',
  color_attributes: [
    { id: '__window_count', label: 'Assigned windows', kind: 'count' },
    { id: 'attribute::priority', label: 'priority', kind: 'categorical' },
  ],
  object_count: 2,
  feature_count: 4,
  window_count: 3,
  feature_columns: [
    'activity.Create Order',
    'activity.Close Order',
    'related_objects.Item',
    'attribute.priority=High',
  ],
  table_preview: [
    {
      object_id: 'O1',
      values: [1, 1, 2, 1],
    },
    {
      object_id: 'O2',
      values: [1, 0, 1, 0],
    },
  ],
  pca: {
    pc1_variance: 1.4,
    pc2_variance: 0.3,
    pc1_explained_ratio: 0.7,
    pc2_explained_ratio: 0.15,
  },
  som: {
    cells: [
      {
        x: 0,
        y: 0,
        label: 'S1-1',
        count: 2,
        color_value: 1,
        color_label: 'priority: High (2)',
        color_kind: 'categorical',
        avg_pc1: -0.4,
        avg_pc2: 0.1,
        dominant_activity: 'Create Order',
      },
      {
        x: 1,
        y: 0,
        label: 'S2-1',
        count: 0,
        color_value: 0,
        color_label: 'priority: n/a',
        color_kind: 'categorical',
        avg_pc1: 0.2,
        avg_pc2: 0.2,
      },
      {
        x: 0,
        y: 1,
        label: 'S1-2',
        count: 1,
        color_value: 0.5,
        color_label: 'priority: Low (1)',
        color_kind: 'categorical',
        avg_pc1: 0.8,
        avg_pc2: -0.1,
        dominant_activity: 'Close Order',
      },
      {
        x: 1,
        y: 1,
        label: 'S2-2',
        count: 0,
        color_value: 0,
        color_label: 'priority: n/a',
        color_kind: 'categorical',
        avg_pc1: 0.6,
        avg_pc2: 0.5,
      },
    ],
    transitions: [
      {
        source_x: 0,
        source_y: 0,
        target_x: 0,
        target_y: 1,
        count: 1,
        distance: 1,
        nearby: true,
      },
    ],
  },
  windows: [
    {
      object_id: 'O1',
      start_event: 'e1',
      end_event: 'e2',
      pc1: -0.4,
      pc2: 0.1,
      cell_x: 0,
      cell_y: 0,
    },
    {
      object_id: 'O1',
      start_event: 'e2',
      end_event: 'e3',
      pc1: 0.8,
      pc2: -0.1,
      cell_x: 0,
      cell_y: 1,
    },
  ],
};

const stateDetectionCellDetail: StateDetectionCellDetail = {
  cell: stateDetectionAnalysis.som.cells[0],
  dfg: traditionalProcessGraph,
  entering_dfg: {
    ...traditionalProcessGraph,
    title: 'Entering Windows: S1-1',
    subtitle: 'Directly-follows graph over windows entering the selected SOM cell',
  },
  exiting_dfg: {
    ...traditionalProcessGraph,
    title: 'Exiting Windows: S1-1',
    subtitle: 'Directly-follows graph over windows exiting the selected SOM cell',
  },
  entering_window_count: 1,
  exiting_window_count: 1,
  entering_windows: [
    {
      object_id: 'O1',
      start_event: 'e1',
      end_event: 'e2',
      source_cell: 'S1-2',
      target_cell: 'S1-1',
      pc1: -0.4,
      pc2: 0.1,
      activities: ['Create Order', 'Close Order'],
    },
  ],
  exiting_windows: [
    {
      object_id: 'O1',
      start_event: 'e2',
      end_event: 'e3',
      source_cell: 'S1-1',
      target_cell: 'S1-2',
      pc1: 0.8,
      pc2: -0.1,
      activities: ['Close Order', 'Archive Order'],
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
    const native = fixture.nativeElement as HTMLElement;
    expect(native.querySelector('.toolbar-left strong')?.textContent).toContain('FLOWVAULT');
    expect(native.querySelector('.drop-title')?.textContent).toContain(
      'Drop an OCEL 2.0 JSON/XML file',
    );
  });

  it('keeps document actions hidden before import', () => {
    const fixture = TestBed.createComponent(App);
    fixture.detectChanges();

    const buttons = Array.from(
      (fixture.nativeElement as HTMLElement).querySelectorAll<HTMLButtonElement>(
        '.toolbar-actions button',
      ),
    );
    expect(buttons.length).toBe(0);
    expect((fixture.nativeElement as HTMLElement).textContent).not.toContain('Export JSON');
  });

  it('shows only the upload area before import', () => {
    const fixture = TestBed.createComponent(App);
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;

    expect(native.querySelector('.upload-page')).toBeTruthy();
    expect(native.querySelectorAll('.summary-card').length).toBe(0);
    expect(native.querySelector('.feature-sidebar')).toBeFalsy();
    expect(native.querySelectorAll('.sample-button').length).toBe(8);
    expect(native.textContent).toContain('Or start with a sample');
  });

  it('imports bundled compressed samples from static assets', async () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      ocelWasm: {
        importDocument(input: ArrayBuffer, formatHint?: string): Promise<unknown>;
      };
    };
    let requestedUrl = '';
    let importedHint: string | undefined;
    let importedByteLength = 0;
    const previousFetch = globalThis.fetch;

    globalThis.fetch = (async (input: RequestInfo | URL) => {
      requestedUrl = input.toString();
      return {
        ok: true,
        arrayBuffer: async () => new Uint8Array([0x1f, 0x8b]).buffer,
      } as Response;
    }) as typeof fetch;
    component.ocelWasm = {
      importDocument: async (input: ArrayBuffer, formatHint?: string) => {
        importedHint = formatHint;
        importedByteLength = input.byteLength;
        return {
          document: {
            filteredObjectCentricDirectlyFollowsGraphJson: () =>
              JSON.stringify(traditionalProcessGraph),
            free: () => undefined,
          },
          summary: importedSummary,
          originalSummary: importedSummary,
          filterOptions: {
            event_types: ['Create Order'],
            object_types: ['Order'],
          },
        };
      },
    };

    try {
      fixture.detectChanges();
      (fixture.nativeElement as HTMLElement).querySelector<HTMLButtonElement>('.sample-button')?.click();
      await fixture.whenStable();
      fixture.detectChanges();
    } finally {
      globalThis.fetch = previousFetch;
    }

    const native = fixture.nativeElement as HTMLElement;
    expect(requestedUrl).toContain('/static/ocel2_compressed/ocel20_example.json.gz');
    expect(importedHint).toBe('json');
    expect(importedByteLength).toBe(2);
    expect(native.textContent).toContain('ocel20_example.json.gz');
    expect(native.textContent).toContain('Statistics');
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
    };

    component.documentHandle = {
      applyStateQuery: () =>
        JSON.stringify({
          attribute: 'state',
          leading_object_type: 'Order',
          assigned_events: 2,
          total_events: 2,
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
