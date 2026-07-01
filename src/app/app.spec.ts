import { TestBed } from '@angular/core/testing';
import { vi } from 'vitest';
import { App } from './app';
import {
  ProcessGraph,
  StateCorrelationResult,
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

const stateCorrelationAnalysis: StateCorrelationResult = {
  object_type: 'Order',
  object_count: 10,
  stateful_object_count: 8,
  state_count: 2,
  feature_count: 2,
  state_distribution: [
    { state: 'Open', count: 5 },
    { state: 'Closed', count: 3 },
  ],
  rows: [
    {
      feature: 'activity.Create Order',
      state: 'Open',
      correlation: 0.82,
      strength: 0.82,
      sample_count: 8,
      state_count: 5,
      mean_in_state: 1.4,
      mean_outside_state: 0.3,
    },
    {
      feature: 'activity.Close Order',
      state: 'Closed',
      correlation: -0.64,
      strength: 0.64,
      sample_count: 8,
      state_count: 3,
      mean_in_state: 0.2,
      mean_outside_state: 1.1,
    },
  ],
};

const transitionKpisAnalysis = {
  object_type: 'Order',
  object_count: 10,
  stateful_object_count: 8,
  state_count: 2,
  states: ['Open', 'Closed'],
  transitions: [
    {
      from_state: 'Open',
      to_state: 'Closed',
      count: 6,
      object_count: 5,
      min_duration_ms: 60_000,
      median_duration_ms: 120_000,
      avg_duration_ms: 150_000,
      max_duration_ms: 240_000,
    },
  ],
  dwell: [
    {
      state: 'Open',
      episode_count: 7,
      object_count: 6,
      total_duration_ms: 600_000,
      min_duration_ms: 60_000,
      median_duration_ms: 120_000,
      avg_duration_ms: 140_000,
      max_duration_ms: 300_000,
    },
  ],
  recovery: [
    {
      from_state: 'Open',
      to_state: 'Closed',
      count: 6,
      object_count: 5,
      min_duration_ms: 60_000,
      median_duration_ms: 120_000,
      avg_duration_ms: 150_000,
      max_duration_ms: 240_000,
    },
  ],
  stuck: [
    {
      object_id: 'O1',
      state: 'Open',
      entered_time_ms: 0,
      last_time_ms: 240_000,
      duration_ms: 240_000,
      event_count: 3,
    },
  ],
};

const objectSearchResult = {
  objects: [{ object_id: 'O1', object_type: 'Order', event_count: 3 }],
};

const lifecycleDetail = {
  object_id: 'O1',
  object_type: 'Order',
  event_count: 3,
  event_min_ms: 0,
  event_max_ms: 240_000,
  state_bands: [
    {
      state: 'Open',
      start_time_ms: 0,
      end_time_ms: 120_000,
      event_count: 2,
      start_event_id: 'e1',
      end_event_id: 'e2',
    },
    {
      state: 'Closed',
      start_time_ms: 240_000,
      end_time_ms: 240_000,
      event_count: 1,
      start_event_id: 'e3',
      end_event_id: 'e3',
    },
  ],
  stock_points: [
    { name: 'Stock After', time_ms: 0, value: 10, event_id: 'e1' },
    { name: 'Stock After', time_ms: 240_000, value: 20, event_id: 'e3' },
  ],
  related_objects: [
    { object_id: 'I1', object_type: 'Item', qualifier: 'contains', event_count: 2 },
  ],
  events: [
    {
      event_id: 'e1',
      event_type: 'Create Order',
      time_ms: 0,
      state: 'Open',
      attributes: [{ name: 'Stock After', value: 10 }],
      related_objects: [{ object_id: 'I1', object_type: 'Item', qualifier: 'contains' }],
    },
    {
      event_id: 'e2',
      event_type: 'Pick Item',
      time_ms: 120_000,
      state: 'Open',
      attributes: [],
      related_objects: [{ object_id: 'I1', object_type: 'Item', qualifier: 'contains' }],
    },
    {
      event_id: 'e3',
      event_type: 'Close Order',
      time_ms: 240_000,
      state: 'Closed',
      attributes: [{ name: 'Stock After', value: 20 }],
      related_objects: [],
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
    localStorage.clear();
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
    expect(buttons.map((button) => button.textContent?.trim())).toEqual(['LLM Config']);
    expect((fixture.nativeElement as HTMLElement).textContent).not.toContain('Export JSON');
  });

  it('exports JSON and XML from a single export menu', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      fileName: { set(value: string): void };
      summary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
    };
    const createObjectUrl = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:export');
    const revokeObjectUrl = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined);
    const clickSpy = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => undefined);

    component.documentHandle = {
      exportJson: () => '{"events":[]}',
      exportXml: () => '<log />',
    };
    component.fileName.set('orders.json');
    component.summary.set(importedSummary);
    component.filterOptions.set({ event_types: [], object_types: [] });
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    const topbarButtons = Array.from(
      native.querySelectorAll<HTMLButtonElement>('.toolbar-actions > button, .toolbar-actions > .toolbar-popover-anchor > button'),
    ).map((button) => button.textContent?.trim());
    expect(topbarButtons).toContain('Export');
    expect(topbarButtons).not.toContain('Export JSON');
    expect(topbarButtons).not.toContain('Export XML');
    expect(native.querySelector('.toolbar-actions input[type="file"]')).toBeFalsy();

    Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-actions button'))
      .find((button) => button.textContent?.trim() === 'Export')
      ?.click();
    fixture.detectChanges();

    expect(native.querySelectorAll('.export-menu button').length).toBe(2);
    expect(native.textContent).toContain('JSON');
    expect(native.textContent).toContain('XML');

    native.querySelector<HTMLButtonElement>('.export-menu button')?.click();
    fixture.detectChanges();
    expect(clickSpy).toHaveBeenCalledTimes(1);
    expect(createObjectUrl).toHaveBeenCalledTimes(1);
    expect(native.querySelector('.export-menu')).toBeFalsy();

    Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-actions button'))
      .find((button) => button.textContent?.trim() === 'Export')
      ?.click();
    fixture.detectChanges();
    native.querySelectorAll<HTMLButtonElement>('.export-menu button')[1]?.click();
    fixture.detectChanges();

    expect(clickSpy).toHaveBeenCalledTimes(2);
    expect(createObjectUrl).toHaveBeenCalledTimes(2);

    createObjectUrl.mockRestore();
    revokeObjectUrl.mockRestore();
    clickSpy.mockRestore();
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

  it('renders feature correlations for the currently applied state', () => {
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
    let correlationRequests = 0;

    component.documentHandle = {
      stateCorrelationsJson: () => {
        correlationRequests += 1;
        return JSON.stringify(stateCorrelationAnalysis);
      },
    };
    component.fileName.set('orders.json');
    component.summary.set(statefulSummary);
    component.originalSummary.set(statefulSummary);
    component.filterOptions.set({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      text_attributes: [{ scope: 'event', name: 'state', values: ['Open', 'Closed'] }],
    });
    component.selectedEventTypes.set(['Create Order', 'Close Order']);
    component.selectedObjectTypes.set(['Order', 'Item']);
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    Array.from(native.querySelectorAll<HTMLButtonElement>('.feature-button'))
      .find((button) => button.textContent?.includes('Correlation'))
      ?.click();
    fixture.detectChanges();

    expect(correlationRequests).toBe(1);
    expect(native.textContent).toContain('Correlation');
    expect(native.textContent).toContain('Feature-State Correlation');
    expect(native.textContent).toContain('activity.Create Order');
    expect(native.textContent).toContain('activity.Close Order');
    expect(native.textContent).toContain('+0.820');
    expect(native.textContent).toContain('-0.640');
    expect(native.textContent).toContain('82%');
    expect(native.textContent).toContain('5 / 8');
    expect(native.querySelector('.correlation-table-scroll')).toBeTruthy();
    expect(
      native.querySelector<HTMLElement>('.correlation-value')?.getAttribute('style'),
    ).toContain('hsl');
    expect(native.querySelector('.correlation-table-panel header span')?.getAttribute('title')).toBe(
      'Open: 5, Closed: 3',
    );

    native.querySelector<HTMLButtonElement>('.correlation-header .ghost-button')?.click();
    fixture.detectChanges();
    expect(correlationRequests).toBe(2);
  });

  it('renders transition KPIs and object lifecycle timelines', () => {
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
    let kpiRequests = 0;
    let searchRequests = 0;
    let lifecycleRequests = 0;

    component.documentHandle = {
      stateTransitionKpisJson: () => {
        kpiRequests += 1;
        return JSON.stringify(transitionKpisAnalysis);
      },
      objectSearchJson: () => {
        searchRequests += 1;
        return JSON.stringify(objectSearchResult);
      },
      objectLifecycleDetailJson: (objectId: string) => {
        lifecycleRequests += 1;
        expect(objectId).toBe('O1');
        return JSON.stringify(lifecycleDetail);
      },
    };
    component.fileName.set('orders.json');
    component.summary.set(statefulSummary);
    component.originalSummary.set(statefulSummary);
    component.filterOptions.set({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      text_attributes: [{ scope: 'event', name: 'state', values: ['Open', 'Closed'] }],
    });
    component.selectedEventTypes.set(['Create Order', 'Close Order']);
    component.selectedObjectTypes.set(['Order', 'Item']);
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    Array.from(native.querySelectorAll<HTMLButtonElement>('.feature-button'))
      .find((button) => button.textContent?.includes('Transition KPIs'))
      ?.click();
    fixture.detectChanges();

    expect(kpiRequests).toBe(1);
    expect(native.textContent).toContain('Transition Matrix');
    expect(native.textContent).toContain('Open -> Closed');
    expect(native.textContent).toContain('Stuck In State');
    expect(native.textContent).toContain('O1');

    Array.from(native.querySelectorAll<HTMLButtonElement>('.feature-button'))
      .find((button) => button.textContent?.includes('Lifecycle Timeline'))
      ?.click();
    fixture.detectChanges();

    expect(searchRequests).toBe(1);
    expect(lifecycleRequests).toBe(1);
    expect(native.textContent).toContain('Lifecycle Timeline');
    expect(native.textContent).toContain('Create Order');
    expect(native.textContent).toContain('Stock After');
    expect(native.textContent).toContain('Related Objects');
    expect(native.querySelector('.lifecycle-timeline')).toBeTruthy();
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

  it('applies global OC-DFG, text attribute, and pattern filters', () => {
    const fixture = TestBed.createComponent(App);
    let filterRequest = '';
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      summary: { set(value: unknown): void };
      originalSummary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
      selectedEventTypes: { set(value: string[]): void };
      selectedObjectTypes: { set(value: string[]): void };
      patternAnalysis: { set(value: unknown): void };
      traditionalOcdfg: { set(value: unknown): void };
    };
    const edgeFilterGraph: ProcessGraph = {
      ...traditionalProcessGraph,
      nodes: [
        {
          ...traditionalProcessGraph.nodes[1],
          id: 'a',
          label: 'Create Order',
          kind: 'activity',
          lines: ['Create Order'],
        },
        {
          ...traditionalProcessGraph.nodes[1],
          id: 'b',
          label: 'Close Order',
          kind: 'activity',
          lines: ['Close Order'],
        },
      ],
      edges: [
        {
          ...traditionalProcessGraph.edges[0],
          id: 'ab',
          source: 'a',
          target: 'b',
        },
      ],
    };

    component.documentHandle = {
      applyFilter: (request: string) => {
        filterRequest = request;
        return JSON.stringify({ ...statefulSummary, events: 1, objects: 1 });
      },
      originalSummaryJson: () => JSON.stringify(statefulSummary),
      statePatternsJson: () => JSON.stringify(patternAnalysis),
      filteredStateAwareObjectCentricDirectlyFollowsGraphJson: () => JSON.stringify(processGraph),
      filteredObjectCentricDirectlyFollowsGraphJson: () => JSON.stringify(edgeFilterGraph),
    };
    component.summary.set(statefulSummary);
    component.originalSummary.set(statefulSummary);
    component.filterOptions.set({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      text_attributes: [{ scope: 'event', name: 'state', values: ['Open', 'Closed'] }],
    });
    component.selectedEventTypes.set(['Create Order', 'Close Order']);
    component.selectedObjectTypes.set(['Order', 'Item']);
    component.patternAnalysis.set(patternAnalysis);
    component.traditionalOcdfg.set(edgeFilterGraph);
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-button'))
      .find((button) => button.textContent?.includes('Filter'))
      ?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.filter-menu button'))
      .find((button) => button.textContent?.includes('OC-DFG Nodes'))
      ?.click();
    fixture.detectChanges();
    native.querySelector<HTMLInputElement>('.filter-modal .filter-options input')?.click();
    fixture.detectChanges();
    native.querySelector<HTMLButtonElement>('.state-modal-footer button:last-child')?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      df_nodes: ['Create Order'],
    });
    native.querySelector<HTMLButtonElement>('.filter-count-button')?.click();
    fixture.detectChanges();
    expect(native.textContent).toContain('OC-DFG nodes 1');

    native.querySelector<HTMLButtonElement>('.filter-chip-remove')?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-button'))
      .find((button) => button.textContent?.includes('Filter'))
      ?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.filter-menu button'))
      .find((button) => button.textContent?.includes('OC-DFG Edges'))
      ?.click();
    fixture.detectChanges();
    native.querySelector<HTMLInputElement>('.filter-modal .filter-options input')?.click();
    fixture.detectChanges();
    native.querySelector<HTMLButtonElement>('.state-modal-footer button:last-child')?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      df_edges: [{ source: 'Create Order', target: 'Close Order' }],
    });

    native.querySelector<HTMLButtonElement>('.filter-count-button')?.click();
    fixture.detectChanges();
    native.querySelector<HTMLButtonElement>('.filter-chip-remove')?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.toolbar-button'))
      .find((button) => button.textContent?.includes('Filter'))
      ?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.filter-menu button'))
      .find((button) => button.textContent?.includes('Text Attributes'))
      ?.click();
    fixture.detectChanges();
    native.querySelector<HTMLInputElement>('.filter-modal .filter-options input')?.click();
    fixture.detectChanges();
    native.querySelector<HTMLButtonElement>('.state-modal-footer button:last-child')?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      text_attributes: [{ scope: 'event', name: 'state', values: ['Open'] }],
    });
    native.querySelector<HTMLButtonElement>('.filter-count-button')?.click();
    fixture.detectChanges();
    expect(native.textContent).toContain('state 1');

    native.querySelector<HTMLButtonElement>('.filter-chip-remove')?.click();
    fixture.detectChanges();
    Array.from(native.querySelectorAll<HTMLButtonElement>('.feature-button'))
      .find((button) => button.textContent?.includes('Patterns'))
      ?.click();
    fixture.detectChanges();
    native
      .querySelector<HTMLButtonElement>('.pattern-panel .graph-toolbar .ghost-button')
      ?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest).patterns).toEqual([
      {
        family: 'intra',
        leading_object_type: 'Order',
        state: 'Open',
        sequence: ['START Open', 'Create Order [Open]', 'END Open'],
        eo_edges: [{ source: 'Create Order [Open]', target: 'Item' }],
        oo_edges: [{ source: 'Order', target: 'Item' }],
      },
    ]);
    native.querySelector<HTMLButtonElement>('.filter-count-button')?.click();
    fixture.detectChanges();
    expect(native.textContent).toContain('Patterns 1');
  });

  it('opens OC-DFG filter dropdowns from graph node and edge clicks', () => {
    const fixture = TestBed.createComponent(App);
    let filterRequest = '';
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      summary: { set(value: unknown): void };
      originalSummary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
      selectedEventTypes: { set(value: string[]): void };
      selectedObjectTypes: { set(value: string[]): void };
      traditionalOcdfg: { set(value: unknown): void };
      activeFeature: { set(value: string): void };
    };
    const edgeFilterGraph: ProcessGraph = {
      ...traditionalProcessGraph,
      nodes: [
        {
          ...traditionalProcessGraph.nodes[1],
          id: 'a',
          label: 'Create Order',
          kind: 'activity',
          lines: ['Create Order'],
        },
        {
          ...traditionalProcessGraph.nodes[1],
          id: 'b',
          label: 'Close Order',
          kind: 'activity',
          lines: ['Close Order'],
        },
      ],
      edges: [
        {
          ...traditionalProcessGraph.edges[0],
          id: 'ab',
          source: 'a',
          target: 'b',
        },
      ],
    };

    component.documentHandle = {
      applyFilter: (request: string) => {
        filterRequest = request;
        return JSON.stringify({ ...importedSummary, events: 1, objects: 1 });
      },
      originalSummaryJson: () => JSON.stringify(importedSummary),
      filteredObjectCentricDirectlyFollowsGraphJson: () => JSON.stringify(edgeFilterGraph),
    };
    component.summary.set(importedSummary);
    component.originalSummary.set(importedSummary);
    component.filterOptions.set({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      text_attributes: [],
    });
    component.selectedEventTypes.set(['Create Order', 'Close Order']);
    component.selectedObjectTypes.set(['Order', 'Item']);
    component.traditionalOcdfg.set(edgeFilterGraph);
    component.activeFeature.set('ocdfg');
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    native
      .querySelector<SVGElement>('g.process-node-activity .process-node-shape')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: 130, clientY: 140 }));
    fixture.detectChanges();

    expect(native.textContent).toContain('Filter objects containing this activity');
    Array.from(native.querySelectorAll<HTMLButtonElement>('.graph-filter-dropdown button'))
      .find((button) => button.textContent?.includes('activity'))
      ?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      df_nodes: ['Create Order'],
    });

    native.querySelector<HTMLButtonElement>('.filter-count-button')?.click();
    fixture.detectChanges();
    native.querySelector<HTMLButtonElement>('.filter-chip-remove')?.click();
    fixture.detectChanges();
    native
      .querySelector<SVGElement>('.process-edge-hitbox.is-filterable')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: 210, clientY: 140 }));
    fixture.detectChanges();

    expect(native.textContent).toContain('Directly-follows edge');
    Array.from(native.querySelectorAll<HTMLButtonElement>('.graph-filter-dropdown button'))
      .find((button) => button.textContent?.includes('edge'))
      ?.click();
    fixture.detectChanges();

    expect(JSON.parse(filterRequest)).toEqual({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      df_edges: [{ source: 'Create Order', target: 'Close Order' }],
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
