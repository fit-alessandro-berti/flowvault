import { Component, computed, inject, signal } from '@angular/core';
import { exportBaseName, formatHintForFile } from './ocel-file';
import { presetsForFile, StateQueryPreset } from './state-query-presets';
import {
  ProcessGraphComponent,
  ProcessGraphEdgeFilterEvent,
  ProcessGraphNodeFilterEvent,
} from './process-graph.component';
import {
  DEFAULT_LLM_PROVIDER,
  LLM_PROVIDERS,
  LlmConfig,
  LlmProviderId,
  providerById,
  requestChatCompletion,
} from './llm';
import {
  OcelDocumentHandle,
  OcelFilterOptions,
  ProcessGraph,
  ProcessGraphSettings,
  StatePattern,
  StatePatternAnalysis,
  StatePatternEdge,
  StateDetectionResult,
  StateDetectionPreviewRow,
  StateDetectionSomCell,
  StateDetectionSomTransition,
  StateDetectionCellDetail,
  StateDetectionColorOption,
  CausalFeatureTableResult,
  StateCorrelationResult,
  StateCorrelationRow,
  TimeFrequencyBucket,
  TimePerformanceSample,
  TimePerspectiveResult,
  CausalFitResult,
  CausalFitNode,
  CausalFitEdge,
  OcelSummary,
  OcelWasmService,
  StateQueryResult,
  FilterTimeBucket,
  TextAttributeOption,
} from './ocel-wasm.service';

interface SummaryCard {
  label: string;
  value: SummaryDisplayValue;
}

interface SummaryDisplayValue {
  current: string;
  original?: string;
  filtered: boolean;
}

type SummaryMetric = keyof Pick<
  OcelSummary,
  | 'event_types'
  | 'object_types'
  | 'events'
  | 'objects'
  | 'e2o_relationships'
  | 'o2o_relationships'
  | 'interned_strings'
  | 'objects_with_lifecycle'
  | 'stateful_events'
>;

interface FilterRequest {
  event_types: string[];
  object_types: string[];
  df_nodes?: string[];
  df_edges?: DfEdgeFilterRequest[];
  time_range?: TimeRangeFilterRequest;
  text_attributes?: TextAttributeFilterRequest[];
  patterns?: PatternFilterRequest[];
}

interface DfEdgeFilterRequest {
  source: string;
  target: string;
}

interface TimeRangeFilterRequest {
  start_ms?: number;
  end_ms?: number;
}

interface TextAttributeFilterRequest {
  scope: 'event' | 'object';
  name: string;
  values: string[];
}

interface PatternFilterRequest {
  family: PatternTab;
  leading_object_type: string;
  state?: string;
  from_state?: string;
  to_state?: string;
  sequence: string[];
  eo_edges: PatternEdgeFilterRequest[];
  oo_edges: PatternEdgeFilterRequest[];
}

interface PatternEdgeFilterRequest {
  source: string;
  target: string;
}

interface DfEdgeOption extends DfEdgeFilterRequest {
  label: string;
}

type FilterDialogKind =
  | 'activities'
  | 'objectTypes'
  | 'dfNodes'
  | 'dfEdges'
  | 'timeframe'
  | 'textAttributes'
  | 'patterns';
type PatternTab = 'intra' | 'inter';
type StateDetectionCellTab = 'dfg' | 'entering' | 'exiting';
type PatternVisualization = 'text' | 'graph';
type FeaturePage =
  | 'statistics'
  | 'stateDetection'
  | 'causalModel'
  | 'ocdfg'
  | 'patterns'
  | 'correlation'
  | 'timePerspective'
  | 'stateAwareOcdfg';
type CausalNodeRole = 'observable' | 'latent' | 'outcome';
type CausalOperation = 'identity' | 'log10' | 'log_e' | 'sqrt';

type GraphFilterMenu =
  | { kind: 'node'; activity: string; x: number; y: number }
  | { kind: 'edge'; source: string; target: string; x: number; y: number };

const SAVED_STATE_PRESET_ID = '__saved_state_expression';
const LLM_STATE_PRESET_ID = '__llm_state_expression';

interface AppliedFilterChip {
  kind: FilterDialogKind;
  label: string;
  description: string;
  removeLabel: string;
}

interface PatternGraphNode {
  id: string;
  lines: string[];
  title: string;
  x: number;
  y: number;
  kind: 'control' | 'change' | 'object';
}

interface PatternGraphEdge {
  id: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  label: string;
  kind: 'df' | 'eo' | 'oo';
}

interface PatternGraph {
  width: number;
  height: number;
  nodeWidth: number;
  nodeHeight: number;
  nodes: PatternGraphNode[];
  edges: PatternGraphEdge[];
}

interface PatternExplorerRow {
  pattern: StatePattern;
  graph: PatternGraph;
}

interface StaticSampleLog {
  label: string;
  detail: string;
  fileName: string;
  path: string;
}

interface CausalModelNode {
  id: string;
  label: string;
  role: CausalNodeRole;
  feature?: string;
  operation: CausalOperation;
}

interface CausalModelEdge {
  source: string;
  target: string;
}

interface CausalFitGraphNode {
  id: string;
  label: string;
  role: CausalNodeRole;
  x: number;
  y: number;
  width: number;
  height: number;
  lines: string[];
}

interface CausalFitGraphEdge {
  id: string;
  source: CausalFitGraphNode;
  target: CausalFitGraphNode;
  edge: CausalFitEdge;
  path: string;
  labelX: number;
  labelY: number;
}

interface CausalFitGraph {
  width: number;
  height: number;
  nodes: CausalFitGraphNode[];
  edges: CausalFitGraphEdge[];
}

interface TimeFrequencySeries {
  state: string;
  color: string;
  path: string;
  areaPath: string;
  latest: number;
}

interface TimeFrequencyChart {
  width: number;
  height: number;
  startLabel: string;
  endLabel: string;
  yTicks: number[];
  series: TimeFrequencySeries[];
}

interface TimeFilterCurve {
  width: number;
  height: number;
  path: string;
  areaPath: string;
  startLabel: string;
  endLabel: string;
  selectedStartX: number;
  selectedEndX: number;
  selectionTop: number;
  selectionBottom: number;
}

interface PerformanceSpectrumPoint {
  sample: TimePerformanceSample;
  x: number;
  y: number;
  radius: number;
}

interface PerformanceSpectrumChart {
  width: number;
  height: number;
  points: PerformanceSpectrumPoint[];
  minLabel: string;
  maxLabel: string;
  medianX: number | null;
}

@Component({
  selector: 'app-root',
  imports: [ProcessGraphComponent],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App {
  private readonly ocelWasm = inject(OcelWasmService);
  private documentHandle?: OcelDocumentHandle;

  protected readonly sampleLogs = STATIC_SAMPLE_LOGS;
  protected readonly causalOperations = CAUSAL_OPERATIONS;
  protected readonly isDragging = signal(false);
  protected readonly isLoading = signal(false);
  protected readonly fileName = signal('');
  protected readonly errorMessage = signal('');
  protected readonly stateMessage = signal('');
  protected readonly isStateDialogOpen = signal(false);
  protected readonly isLlmConfigOpen = signal(false);
  protected readonly selectedPresetId = signal('');
  protected readonly selectedLeadingObjectType = signal('');
  protected readonly stateQueryDraft = signal('');
  protected readonly persistedStateExpression = signal(
    readStoredString(STATE_EXPRESSION_STORAGE_KEY),
  );
  protected readonly llmProviders = LLM_PROVIDERS;
  protected readonly llmProvider = signal<LlmProviderId>(loadLlmConfig().provider);
  protected readonly llmModel = signal(loadLlmConfig().model);
  protected readonly llmApiKey = signal(loadLlmConfig().apiKey);
  protected readonly llmStatus = signal('');
  protected readonly isTestingLlm = signal(false);
  protected readonly isGeneratingStateExpression = signal(false);
  protected readonly llmStatePrompt = signal(DEFAULT_LLM_STATE_PROMPT);
  protected readonly summary = signal<OcelSummary | null>(null);
  protected readonly originalSummary = signal<OcelSummary | null>(null);
  protected readonly filterOptions = signal<OcelFilterOptions>({
    event_types: [],
    object_types: [],
    text_attributes: [],
    time_buckets: [],
  });
  protected readonly selectedEventTypes = signal<string[]>([]);
  protected readonly selectedObjectTypes = signal<string[]>([]);
  protected readonly selectedDfNodes = signal<string[]>([]);
  protected readonly selectedDfEdges = signal<DfEdgeFilterRequest[]>([]);
  protected readonly selectedTimeRange = signal<TimeRangeFilterRequest | null>(null);
  protected readonly selectedTextAttribute = signal<TextAttributeFilterRequest | null>(null);
  protected readonly selectedPatternFilters = signal<PatternFilterRequest[]>([]);
  protected readonly draftEventTypes = signal<string[]>([]);
  protected readonly draftObjectTypes = signal<string[]>([]);
  protected readonly draftDfNodes = signal<string[]>([]);
  protected readonly draftDfEdges = signal<DfEdgeFilterRequest[]>([]);
  protected readonly draftTimeStart = signal('');
  protected readonly draftTimeEnd = signal('');
  protected readonly isSelectingTimeRange = signal(false);
  private timeSelectionAnchorMs: number | null = null;
  protected readonly draftTextAttributeKey = signal('');
  protected readonly draftTextAttributeValues = signal<string[]>([]);
  protected readonly filterDialog = signal<FilterDialogKind | null>(null);
  protected readonly isFilterMenuOpen = signal(false);
  protected readonly isExportMenuOpen = signal(false);
  protected readonly isFilterChainOpen = signal(false);
  protected readonly graphFilterMenu = signal<GraphFilterMenu | null>(null);
  protected readonly stateDetectionObjectType = signal('');
  protected readonly stateDetectionWindowSize = signal(4);
  protected readonly stateDetectionSomWidth = signal(3);
  protected readonly stateDetectionSomHeight = signal(3);
  protected readonly stateDetectionColorAttribute = signal('__window_count');
  protected readonly stateDetectionColorOptions = signal<StateDetectionColorOption[]>(
    DEFAULT_STATE_DETECTION_COLOR_OPTIONS,
  );
  protected readonly stateDetectionAnalysis = signal<StateDetectionResult | null>(null);
  protected readonly stateDetectionCellDetail = signal<StateDetectionCellDetail | null>(null);
  protected readonly stateDetectionCellTab = signal<StateDetectionCellTab>('dfg');
  protected readonly causalObjectType = signal('');
  protected readonly causalFeatureTable = signal<CausalFeatureTableResult | null>(null);
  protected readonly causalNodes = signal<CausalModelNode[]>([]);
  protected readonly causalEdges = signal<CausalModelEdge[]>([]);
  protected readonly causalLatentDraft = signal('');
  protected readonly causalFit = signal<CausalFitResult | null>(null);
  protected readonly causalMessage = signal('');
  protected readonly isGeneratingCausalModel = signal(false);
  protected readonly patternAnalysis = signal<StatePatternAnalysis | null>(null);
  protected readonly stateCorrelation = signal<StateCorrelationResult | null>(null);
  protected readonly timePerspective = signal<TimePerspectiveResult | null>(null);
  protected readonly timePerspectiveObjectType = signal('');
  protected readonly timePerspectiveFromState = signal('');
  protected readonly timePerspectiveToState = signal('');
  protected readonly timePerspectiveRoundtrip = signal(false);
  protected readonly stateAwareOcdfg = signal<ProcessGraph | null>(null);
  protected readonly traditionalOcdfg = signal<ProcessGraph | null>(null);
  protected readonly stateAwareOcdfgSettings = signal<ProcessGraphSettings>(emptyGraphSettings());
  protected readonly traditionalOcdfgSettings = signal<ProcessGraphSettings>(emptyGraphSettings());
  protected readonly selectedIntraPatternId = signal('');
  protected readonly selectedInterPatternId = signal('');
  protected readonly activePatternTab = signal<PatternTab>('intra');
  protected readonly intraVisualization = signal<PatternVisualization>('text');
  protected readonly interVisualization = signal<PatternVisualization>('text');
  protected readonly isPatternExplorerOpen = signal(false);
  protected readonly fullScreenPattern = signal<StatePattern | null>(null);
  protected readonly activeFeature = signal<FeaturePage>('statistics');
  protected readonly hasDocument = computed(() => this.summary() !== null);
  protected readonly hasAppliedState = computed(
    () => (this.originalSummary()?.stateful_events ?? this.summary()?.stateful_events ?? 0) > 0,
  );
  protected readonly isFilterApplied = computed(
    () =>
      this.selectedEventTypes().length !== this.filterOptions().event_types.length ||
      this.selectedObjectTypes().length !== this.filterOptions().object_types.length ||
      this.selectedDfNodes().length > 0 ||
      this.selectedDfEdges().length > 0 ||
      this.selectedTimeRange() !== null ||
      this.selectedTextAttribute() !== null ||
      this.selectedPatternFilters().length > 0,
  );
  protected readonly stateQueryPresets = computed(() => presetsForFile(this.fileName()));
  protected readonly isLlmStateMode = computed(
    () => this.selectedPresetId() === LLM_STATE_PRESET_ID,
  );
  protected readonly leadingObjectTypeOptions = computed(() => {
    const selected = this.selectedObjectTypes();
    return selected.length > 0 ? selected : this.filterOptions().object_types;
  });
  protected readonly appliedFilters = computed<AppliedFilterChip[]>(() => {
    const options = this.filterOptions();
    const chips: AppliedFilterChip[] = [];

    if (this.selectedEventTypes().length < options.event_types.length) {
      chips.push({
        kind: 'activities',
        label: `Activities ${this.selectedEventTypes().length}/${options.event_types.length}`,
        description: filterDescription('Selected activities', this.selectedEventTypes()),
        removeLabel: 'Remove activity filter',
      });
    }

    if (this.selectedObjectTypes().length < options.object_types.length) {
      chips.push({
        kind: 'objectTypes',
        label: `Object types ${this.selectedObjectTypes().length}/${options.object_types.length}`,
        description: filterDescription('Selected object types', this.selectedObjectTypes()),
        removeLabel: 'Remove object type filter',
      });
    }

    if (this.selectedDfNodes().length > 0) {
      chips.push({
        kind: 'dfNodes',
        label: `OC-DFG nodes ${this.selectedDfNodes().length}`,
        description: filterDescription('Objects containing activities', this.selectedDfNodes()),
        removeLabel: 'Remove OC-DFG node filter',
      });
    }

    if (this.selectedDfEdges().length > 0) {
      const labels = this.selectedDfEdges().map((edge) => edgeLabel(edge));
      chips.push({
        kind: 'dfEdges',
        label: `OC-DFG edges ${this.selectedDfEdges().length}`,
        description: filterDescription('Objects containing directly-follows edges', labels),
        removeLabel: 'Remove OC-DFG edge filter',
      });
    }

    const timeRange = this.selectedTimeRange();
    if (timeRange) {
      chips.push({
        kind: 'timeframe',
        label: 'Timeframe',
        description: timeRangeLabel(timeRange),
        removeLabel: 'Remove timeframe filter',
      });
    }

    const textAttribute = this.selectedTextAttribute();
    if (textAttribute && textAttribute.values.length > 0) {
      chips.push({
        kind: 'textAttributes',
        label: `${textAttribute.name} ${textAttribute.values.length}`,
        description: filterDescription(
          `${textAttribute.scope} attribute ${textAttribute.name}`,
          textAttribute.values,
        ),
        removeLabel: 'Remove text attribute filter',
      });
    }

    if (this.selectedPatternFilters().length > 0) {
      chips.push({
        kind: 'patterns',
        label: `Patterns ${this.selectedPatternFilters().length}`,
        description: filterDescription(
          'Objects matching selected state patterns',
          this.selectedPatternFilters().map(patternFilterLabel),
        ),
        removeLabel: 'Remove pattern filter',
      });
    }

    return chips;
  });
  protected readonly dfEdgeOptions = computed<DfEdgeOption[]>(() => {
    const graph = this.traditionalOcdfg();
    if (!graph) {
      return [];
    }
    const nodeById = new Map(graph.nodes.map((node) => [node.id, node]));
    const seen = new Set<string>();
    const options: DfEdgeOption[] = [];
    for (const edge of graph.edges) {
      const source = nodeById.get(edge.source);
      const target = nodeById.get(edge.target);
      if (!source || !target || source.kind !== 'activity' || target.kind !== 'activity') {
        continue;
      }
      const option = {
        source: source.label,
        target: target.label,
        label: `${source.label} -> ${target.label}`,
      };
      const key = edgeKey(option);
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      options.push(option);
    }
    return options;
  });
  protected readonly intraPatterns = computed(() => this.patternAnalysis()?.intra ?? []);
  protected readonly interPatterns = computed(() => this.patternAnalysis()?.inter ?? []);
  protected readonly selectedIntraPattern = computed(() =>
    selectedPattern(this.intraPatterns(), this.selectedIntraPatternId()),
  );
  protected readonly selectedInterPattern = computed(() =>
    selectedPattern(this.interPatterns(), this.selectedInterPatternId()),
  );
  protected readonly patternExplorerRows = computed<PatternExplorerRow[]>(() =>
    [...this.intraPatterns(), ...this.interPatterns()]
      .sort(
        (left, right) =>
          right.support - left.support ||
          right.mass - left.mass ||
          left.label.localeCompare(right.label),
      )
      .map((pattern) => ({
        pattern,
        graph: this.patternGraph(pattern, 'compact'),
      })),
  );
  protected readonly summaryCards = computed<SummaryCard[]>(() => {
    const summary = this.summary();

    return [
      {
        label: 'Events',
        value: summary ? this.summaryDisplayValue('events') : emptySummaryValue(),
      },
      {
        label: 'Objects',
        value: summary ? this.summaryDisplayValue('objects') : emptySummaryValue(),
      },
      {
        label: 'E2O',
        value: summary ? this.summaryDisplayValue('e2o_relationships') : emptySummaryValue(),
      },
      {
        label: 'O2O',
        value: summary ? this.summaryDisplayValue('o2o_relationships') : emptySummaryValue(),
      },
    ];
  });
  protected readonly causalObservableNodes = computed(() =>
    this.causalNodes().filter((node) => node.role === 'observable'),
  );
  protected readonly causalLatentNodes = computed(() =>
    this.causalNodes().filter((node) => node.role === 'latent'),
  );
  protected readonly causalOutcomeNodes = computed(() =>
    this.causalNodes().filter((node) => node.role === 'outcome'),
  );
  protected readonly canFitCausalModel = computed(
    () =>
      this.causalObservableNodes().length > 0 &&
      this.causalLatentNodes().length > 0 &&
      this.causalOutcomeNodes().length > 0 &&
      this.causalEdges().length > 0,
  );
  protected readonly causalFitGraph = computed(() => {
    const fit = this.causalFit();
    return fit ? causalFitGraph(fit) : null;
  });
  protected readonly timeFrequencyChart = computed(() => {
    const analysis = this.timePerspective();
    return analysis ? timeFrequencyChart(analysis.buckets, analysis.states) : null;
  });
  protected readonly timeFilterCurve = computed(() => {
    const options = this.filterOptions();
    return timeFilterCurve(
      options.time_buckets,
      fromDateTimeLocalInput(this.draftTimeStart()),
      fromDateTimeLocalInput(this.draftTimeEnd()),
    );
  });
  protected readonly performanceSpectrumChart = computed(() => {
    const analysis = this.timePerspective();
    return analysis ? performanceSpectrumChart(analysis.performance.samples) : null;
  });

  async onFileSelected(event: Event): Promise<void> {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';

    if (file) {
      await this.importFile(file);
    }
  }

  onDragOver(event: DragEvent): void {
    event.preventDefault();
    this.isDragging.set(true);
  }

  onDragLeave(event: DragEvent): void {
    if (event.currentTarget === event.target) {
      this.isDragging.set(false);
    }
  }

  async onDrop(event: DragEvent): Promise<void> {
    event.preventDefault();
    this.isDragging.set(false);

    const file = event.dataTransfer?.files?.[0];
    if (file) {
      await this.importFile(file);
    }
  }

  protected async importSampleLog(sample: StaticSampleLog): Promise<void> {
    await this.importSource(sample.fileName, async () => {
      const response = await fetch(new URL(sample.path, document.baseURI));
      if (!response.ok) {
        throw new Error(`Could not load bundled sample '${sample.fileName}'.`);
      }
      return response.arrayBuffer();
    });
  }

  exportJson(): void {
    this.isExportMenuOpen.set(false);
    this.exportDocument('json');
  }

  exportXml(): void {
    this.isExportMenuOpen.set(false);
    this.exportDocument('xml');
  }

  protected openLlmConfig(): void {
    this.llmStatus.set('');
    this.isExportMenuOpen.set(false);
    this.isFilterMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.isLlmConfigOpen.set(true);
  }

  protected closeLlmConfig(): void {
    this.isLlmConfigOpen.set(false);
  }

  protected onLlmProviderChange(event: Event): void {
    const provider = providerById((event.target as HTMLSelectElement).value);
    this.llmProvider.set(provider.id);
    this.llmModel.set(provider.defaultModel);
    this.llmStatus.set('');
  }

  protected onLlmModelChange(event: Event): void {
    this.llmModel.set((event.target as HTMLInputElement).value);
    this.llmStatus.set('');
  }

  protected onLlmApiKeyChange(event: Event): void {
    this.llmApiKey.set((event.target as HTMLInputElement).value);
    this.llmStatus.set('');
  }

  protected saveLlmConfig(): void {
    writeStoredJson(LLM_CONFIG_STORAGE_KEY, this.currentLlmConfig());
    this.llmStatus.set('Configuration saved.');
  }

  protected async testLlmConfig(): Promise<void> {
    this.llmStatus.set('');
    if (!this.currentLlmConfig().apiKey.trim()) {
      this.llmStatus.set('API key is required.');
      return;
    }

    this.isTestingLlm.set(true);
    try {
      const response = await requestChatCompletion(this.currentLlmConfig(), [
        {
          role: 'system',
          content: 'Respond with OK.',
        },
        {
          role: 'user',
          content: 'Connection test.',
        },
      ]);
      this.llmStatus.set(`Test succeeded: ${response.slice(0, 80)}`);
    } catch (error) {
      this.llmStatus.set(errorToMessage(error));
    } finally {
      this.isTestingLlm.set(false);
    }
  }

  protected setActiveFeature(feature: FeaturePage): void {
    if (
      (feature === 'patterns' ||
        feature === 'correlation' ||
        feature === 'timePerspective' ||
        feature === 'stateAwareOcdfg') &&
      !this.hasAppliedState()
    ) {
      return;
    }

    this.activeFeature.set(feature);
    this.graphFilterMenu.set(null);
    if (feature === 'stateDetection' && !this.stateDetectionAnalysis()) {
      this.loadStateDetection();
    }
    if (feature === 'causalModel' && !this.causalFeatureTable()) {
      this.loadCausalFeatureTable();
    }
    if (feature === 'correlation' && !this.stateCorrelation()) {
      this.loadStateCorrelation();
    }
    if (feature === 'timePerspective' && !this.timePerspective()) {
      this.loadTimePerspective();
    }
  }

  protected toggleFilterMenu(): void {
    if (!this.hasDocument()) {
      return;
    }

    this.isFilterMenuOpen.update((isOpen) => !isOpen);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
  }

  protected toggleExportMenu(): void {
    if (!this.hasDocument()) {
      return;
    }

    this.isExportMenuOpen.update((isOpen) => !isOpen);
    this.isFilterMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
  }

  protected toggleFilterChain(): void {
    if (this.appliedFilters().length === 0) {
      return;
    }

    this.isFilterChainOpen.update((isOpen) => !isOpen);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.graphFilterMenu.set(null);
  }

  openStateDialog(): void {
    if (!this.documentHandle) {
      return;
    }

    const presets = this.stateQueryPresets();
    const selectedPreset =
      presets.find((preset) => preset.id === this.selectedPresetId()) ?? presets[0];

    if (selectedPreset) {
      this.selectStatePreset(selectedPreset);
    } else {
      this.ensureLeadingObjectTypeSelection();
    }

    this.errorMessage.set('');
    this.isExportMenuOpen.set(false);
    this.isFilterMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.isStateDialogOpen.set(true);
  }

  closeStateDialog(): void {
    this.isStateDialogOpen.set(false);
  }

  selectStatePreset(preset: StateQueryPreset): void {
    const leadingObjectType = this.validLeadingObjectType(preset.leadingObjectType);
    this.selectedPresetId.set(preset.id);
    this.selectedLeadingObjectType.set(leadingObjectType);
    this.stateQueryDraft.set(withLeadingObjectTypeClause(preset.query, leadingObjectType));
  }

  protected selectPersistedStateExpression(): void {
    const expression = this.persistedStateExpression();
    if (!expression) {
      return;
    }

    this.ensureLeadingObjectTypeSelection();
    this.selectedPresetId.set(SAVED_STATE_PRESET_ID);
    this.stateQueryDraft.set(
      withLeadingObjectTypeClause(expression, this.selectedLeadingObjectType()),
    );
  }

  protected selectLlmStateExpression(): void {
    this.ensureLeadingObjectTypeSelection();
    this.selectedPresetId.set(LLM_STATE_PRESET_ID);
    if (!this.stateQueryDraft().trim()) {
      this.stateQueryDraft.set(defaultStateQuery(this.selectedLeadingObjectType()));
    }
  }

  onStateQueryDraftChange(event: Event): void {
    this.stateQueryDraft.set((event.target as HTMLTextAreaElement).value);
  }

  protected onLlmStatePromptChange(event: Event): void {
    this.llmStatePrompt.set((event.target as HTMLTextAreaElement).value);
  }

  onLeadingObjectTypeChange(event: Event): void {
    const leadingObjectType = (event.target as HTMLSelectElement).value;
    this.selectedLeadingObjectType.set(leadingObjectType);
    this.stateQueryDraft.set(
      withLeadingObjectTypeClause(this.stateQueryDraft(), leadingObjectType),
    );
  }

  applyStateQuery(): void {
    if (!this.documentHandle) {
      return;
    }

    this.errorMessage.set('');
    this.stateMessage.set('');
    this.ensureLeadingObjectTypeSelection();
    const query = withLeadingObjectTypeClause(
      this.stateQueryDraft(),
      this.selectedLeadingObjectType(),
    );
    this.stateQueryDraft.set(query);

    try {
      const result = JSON.parse(this.documentHandle.applyStateQuery(query)) as StateQueryResult;
      this.persistStateExpression(query);
      this.filterOptions.set(
        JSON.parse(this.documentHandle.filterOptionsJson()) as OcelFilterOptions,
      );
      this.summary.set(JSON.parse(this.documentHandle.summaryJson()) as OcelSummary);
      this.originalSummary.set(
        JSON.parse(this.documentHandle.originalSummaryJson()) as OcelSummary,
      );
      this.stateCorrelation.set(null);
      this.timePerspective.set(null);
      this.loadStatePatterns();
      this.activeFeature.set('patterns');
      this.stateMessage.set(
        `Added ${result.attribute} for ${result.leading_object_type} to ${result.assigned_events.toLocaleString()} of ${result.total_events.toLocaleString()} events.`,
      );
      this.isStateDialogOpen.set(false);
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  }

  protected async generateStateExpressionWithLlm(): Promise<void> {
    if (!this.documentHandle) {
      return;
    }

    const config = this.currentLlmConfig();
    if (!config.apiKey.trim()) {
      this.errorMessage.set('Configure and save an LLM API key first.');
      this.openLlmConfig();
      return;
    }

    this.errorMessage.set('');
    this.stateMessage.set('');
    this.ensureLeadingObjectTypeSelection();
    this.selectedPresetId.set(LLM_STATE_PRESET_ID);
    this.isGeneratingStateExpression.set(true);

    try {
      const expression = await requestChatCompletion(config, [
        {
          role: 'system',
          content:
            'You generate Flowvault state expressions for object-centric event logs. Return only one valid expression and no markdown.',
        },
        {
          role: 'user',
          content: this.buildLlmStatePrompt(),
        },
      ]);
      this.stateQueryDraft.set(
        withLeadingObjectTypeClause(
          extractStateExpression(expression),
          this.selectedLeadingObjectType(),
        ),
      );
      this.stateMessage.set('LLM state expression generated.');
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    } finally {
      this.isGeneratingStateExpression.set(false);
    }
  }

  private async importFile(file: File): Promise<void> {
    await this.importSource(file.name, () => file.arrayBuffer());
  }

  private async importSource(
    fileName: string,
    readInput: () => Promise<ArrayBuffer>,
  ): Promise<void> {
    this.isLoading.set(true);
    this.errorMessage.set('');

    try {
      const input = await readInput();
      const imported = await this.ocelWasm.importDocument(input, formatHintForFile(fileName));

      this.documentHandle?.free();
      this.documentHandle = imported.document;
      this.fileName.set(fileName);
      this.summary.set(imported.summary);
      this.originalSummary.set(imported.originalSummary);
      this.filterOptions.set(imported.filterOptions);
      this.selectedEventTypes.set(imported.filterOptions.event_types);
      this.selectedObjectTypes.set(imported.filterOptions.object_types);
      this.selectedDfNodes.set([]);
      this.selectedDfEdges.set([]);
      this.selectedTimeRange.set(null);
      this.selectedTextAttribute.set(null);
      this.selectedPatternFilters.set([]);
      this.draftEventTypes.set(imported.filterOptions.event_types);
      this.draftObjectTypes.set(imported.filterOptions.object_types);
      this.draftDfNodes.set([]);
      this.draftDfEdges.set([]);
      this.draftTimeStart.set('');
      this.draftTimeEnd.set('');
      this.draftTextAttributeKey.set('');
      this.draftTextAttributeValues.set([]);
      this.filterDialog.set(null);
      this.isFilterMenuOpen.set(false);
      this.isExportMenuOpen.set(false);
      this.isFilterChainOpen.set(false);
      this.graphFilterMenu.set(null);
      this.stateMessage.set('');
      this.patternAnalysis.set(null);
      this.stateCorrelation.set(null);
      this.timePerspective.set(null);
      this.timePerspectiveObjectType.set(imported.filterOptions.object_types[0] ?? '');
      this.timePerspectiveFromState.set('');
      this.timePerspectiveToState.set('');
      this.timePerspectiveRoundtrip.set(false);
      this.stateAwareOcdfg.set(null);
      this.traditionalOcdfg.set(null);
      this.stateDetectionAnalysis.set(null);
      this.stateDetectionObjectType.set(imported.filterOptions.object_types[0] ?? '');
      this.stateDetectionWindowSize.set(4);
      this.stateDetectionSomWidth.set(3);
      this.stateDetectionSomHeight.set(3);
      this.stateDetectionColorAttribute.set('__window_count');
      this.stateDetectionColorOptions.set(DEFAULT_STATE_DETECTION_COLOR_OPTIONS);
      this.stateDetectionCellDetail.set(null);
      this.stateDetectionCellTab.set('dfg');
      this.causalObjectType.set(imported.filterOptions.object_types[0] ?? '');
      this.causalFeatureTable.set(null);
      this.causalNodes.set([]);
      this.causalEdges.set([]);
      this.causalLatentDraft.set('');
      this.causalFit.set(null);
      this.causalMessage.set('');
      this.isGeneratingCausalModel.set(false);
      this.resetGraphSettings(imported.filterOptions.object_types);
      this.loadTraditionalOcdfg();
      this.selectedIntraPatternId.set('');
      this.selectedInterPatternId.set('');
      this.activePatternTab.set('intra');
      this.activeFeature.set('statistics');
      this.fullScreenPattern.set(null);
      this.isStateDialogOpen.set(false);
      this.initializeStatePresetForFile(fileName);
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
      this.summary.set(null);
      this.originalSummary.set(null);
      this.filterOptions.set({
        event_types: [],
        object_types: [],
        text_attributes: [],
        time_buckets: [],
      });
      this.selectedEventTypes.set([]);
      this.selectedObjectTypes.set([]);
      this.selectedDfNodes.set([]);
      this.selectedDfEdges.set([]);
      this.selectedTimeRange.set(null);
      this.selectedTextAttribute.set(null);
      this.selectedPatternFilters.set([]);
      this.draftEventTypes.set([]);
      this.draftObjectTypes.set([]);
      this.draftDfNodes.set([]);
      this.draftDfEdges.set([]);
      this.draftTimeStart.set('');
      this.draftTimeEnd.set('');
      this.draftTextAttributeKey.set('');
      this.draftTextAttributeValues.set([]);
      this.filterDialog.set(null);
      this.isFilterMenuOpen.set(false);
      this.isExportMenuOpen.set(false);
      this.isFilterChainOpen.set(false);
      this.graphFilterMenu.set(null);
      this.selectedLeadingObjectType.set('');
      this.fileName.set(fileName);
      this.documentHandle?.free();
      this.documentHandle = undefined;
      this.stateMessage.set('');
      this.patternAnalysis.set(null);
      this.stateCorrelation.set(null);
      this.timePerspective.set(null);
      this.timePerspectiveObjectType.set('');
      this.timePerspectiveFromState.set('');
      this.timePerspectiveToState.set('');
      this.timePerspectiveRoundtrip.set(false);
      this.stateAwareOcdfg.set(null);
      this.traditionalOcdfg.set(null);
      this.stateDetectionAnalysis.set(null);
      this.stateDetectionObjectType.set('');
      this.stateDetectionColorAttribute.set('__window_count');
      this.stateDetectionColorOptions.set(DEFAULT_STATE_DETECTION_COLOR_OPTIONS);
      this.stateDetectionCellDetail.set(null);
      this.stateDetectionCellTab.set('dfg');
      this.causalObjectType.set('');
      this.causalFeatureTable.set(null);
      this.causalNodes.set([]);
      this.causalEdges.set([]);
      this.causalLatentDraft.set('');
      this.causalFit.set(null);
      this.causalMessage.set('');
      this.isGeneratingCausalModel.set(false);
      this.resetGraphSettings([]);
      this.selectedIntraPatternId.set('');
      this.selectedInterPatternId.set('');
      this.activePatternTab.set('intra');
      this.activeFeature.set('statistics');
      this.fullScreenPattern.set(null);
      this.isStateDialogOpen.set(false);
    } finally {
      this.isLoading.set(false);
    }
  }

  private exportDocument(format: 'json' | 'xml'): void {
    if (!this.documentHandle) {
      return;
    }

    try {
      const content =
        format === 'json' ? this.documentHandle.exportJson() : this.documentHandle.exportXml();
      const mimeType = format === 'json' ? 'application/json' : 'application/xml';
      this.downloadNamed(content, mimeType, `${exportBaseName(this.fileName())}.${format}`);
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  }

  private downloadNamed(content: string, mimeType: string, fileName: string): void {
    const blob = new Blob([content], { type: `${mimeType};charset=utf-8` });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');

    anchor.href = url;
    anchor.download = fileName;
    anchor.click();
    URL.revokeObjectURL(url);
  }

  private initializeStatePresetForFile(fileName: string): void {
    const preset = presetsForFile(fileName)[0];
    if (preset) {
      this.selectStatePreset(preset);
      return;
    }

    this.selectedPresetId.set('');
    this.ensureLeadingObjectTypeSelection();
    this.stateQueryDraft.set(defaultStateQuery(this.selectedLeadingObjectType()));
  }

  private ensureLeadingObjectTypeSelection(): void {
    this.selectedLeadingObjectType.set(
      this.validLeadingObjectType(this.selectedLeadingObjectType()),
    );
  }

  private validLeadingObjectType(candidate: string): string {
    const options = this.leadingObjectTypeOptions();
    if (candidate && options.includes(candidate)) {
      return candidate;
    }
    return options[0] ?? this.filterOptions().object_types[0] ?? candidate;
  }

  protected selectIntraPattern(event: Event): void {
    this.selectedIntraPatternId.set((event.target as HTMLSelectElement).value);
  }

  protected selectInterPattern(event: Event): void {
    this.selectedInterPatternId.set((event.target as HTMLSelectElement).value);
  }

  protected openActivityFilterDialog(): void {
    this.draftEventTypes.set([...this.selectedEventTypes()]);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.filterDialog.set('activities');
  }

  protected openObjectTypeFilterDialog(): void {
    this.draftObjectTypes.set([...this.selectedObjectTypes()]);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.filterDialog.set('objectTypes');
  }

  protected openDfNodeFilterDialog(): void {
    this.draftDfNodes.set([...this.selectedDfNodes()]);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.filterDialog.set('dfNodes');
  }

  protected openDfEdgeFilterDialog(): void {
    this.draftDfEdges.set([...this.selectedDfEdges()]);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.filterDialog.set('dfEdges');
  }

  protected openTimeframeFilterDialog(): void {
    const selected = this.selectedTimeRange();
    this.draftTimeStart.set(
      toDateTimeLocalInput(selected?.start_ms ?? this.filterOptions().time_min_ms),
    );
    this.draftTimeEnd.set(
      toDateTimeLocalInput(selected?.end_ms ?? this.filterOptions().time_max_ms),
    );
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.filterDialog.set('timeframe');
  }

  protected openTextAttributeFilterDialog(): void {
    const selected = this.selectedTextAttribute();
    const first = selected ?? this.defaultTextAttributeFilter();
    this.draftTextAttributeKey.set(first ? textAttributeKey(first) : '');
    this.draftTextAttributeValues.set(first?.values ?? []);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.filterDialog.set('textAttributes');
  }

  protected closeFilterDialog(): void {
    this.filterDialog.set(null);
  }

  protected toggleDraftEventType(eventType: string, event: Event): void {
    this.draftEventTypes.set(
      toggleSelection(
        this.draftEventTypes(),
        eventType,
        (event.target as HTMLInputElement).checked,
      ),
    );
  }

  protected toggleDraftObjectType(objectType: string, event: Event): void {
    this.draftObjectTypes.set(
      toggleSelection(
        this.draftObjectTypes(),
        objectType,
        (event.target as HTMLInputElement).checked,
      ),
    );
  }

  protected toggleDraftDfNode(activity: string, event: Event): void {
    this.draftDfNodes.set(
      toggleSelection(this.draftDfNodes(), activity, (event.target as HTMLInputElement).checked),
    );
  }

  protected toggleDraftDfEdge(edge: DfEdgeFilterRequest, event: Event): void {
    const checked = (event.target as HTMLInputElement).checked;
    const current = this.draftDfEdges();
    const normalizedEdge = { source: edge.source, target: edge.target };
    this.draftDfEdges.set(
      checked
        ? [...current, normalizedEdge].filter(uniqueEdges)
        : current.filter((candidate) => !sameEdge(candidate, normalizedEdge)),
    );
  }

  protected onDraftTextAttributeChange(event: Event): void {
    const key = (event.target as HTMLSelectElement).value;
    this.draftTextAttributeKey.set(key);
    this.draftTextAttributeValues.set([]);
  }

  protected onDraftTimeStartChange(event: Event): void {
    this.draftTimeStart.set((event.target as HTMLInputElement).value);
  }

  protected onDraftTimeEndChange(event: Event): void {
    this.draftTimeEnd.set((event.target as HTMLInputElement).value);
  }

  protected resetDraftTimeframe(): void {
    this.draftTimeStart.set(toDateTimeLocalInput(this.filterOptions().time_min_ms));
    this.draftTimeEnd.set(toDateTimeLocalInput(this.filterOptions().time_max_ms));
  }

  protected startTimeRangeSelection(event: PointerEvent, curve: TimeFilterCurve): void {
    const timeMs = this.timeMsFromChartEvent(event, curve);
    if (timeMs === null) {
      return;
    }

    event.preventDefault();
    (event.currentTarget as SVGSVGElement).setPointerCapture(event.pointerId);
    this.timeSelectionAnchorMs = timeMs;
    this.isSelectingTimeRange.set(true);
    this.updateDraftTimeRangeFromSelection(timeMs, timeMs);
  }

  protected moveTimeRangeSelection(event: PointerEvent, curve: TimeFilterCurve): void {
    if (!this.isSelectingTimeRange() || this.timeSelectionAnchorMs === null) {
      return;
    }
    const timeMs = this.timeMsFromChartEvent(event, curve);
    if (timeMs === null) {
      return;
    }
    event.preventDefault();
    this.updateDraftTimeRangeFromSelection(this.timeSelectionAnchorMs, timeMs);
  }

  protected endTimeRangeSelection(event: PointerEvent, curve: TimeFilterCurve): void {
    if (!this.isSelectingTimeRange()) {
      return;
    }
    this.moveTimeRangeSelection(event, curve);
    if ((event.currentTarget as SVGSVGElement).hasPointerCapture(event.pointerId)) {
      (event.currentTarget as SVGSVGElement).releasePointerCapture(event.pointerId);
    }
    this.timeSelectionAnchorMs = null;
    this.isSelectingTimeRange.set(false);
  }

  private timeMsFromChartEvent(event: PointerEvent, curve: TimeFilterCurve): number | null {
    const options = this.filterOptions();
    const minMs = options.time_min_ms;
    const maxMs = options.time_max_ms;
    if (minMs === undefined || maxMs === undefined) {
      return null;
    }
    const rect = (event.currentTarget as SVGSVGElement).getBoundingClientRect();
    const ratio = Math.min(Math.max((event.clientX - rect.left) / rect.width, 0), 1);
    return Math.round(minMs + ratio * (maxMs - minMs));
  }

  private updateDraftTimeRangeFromSelection(startMs: number, endMs: number): void {
    const start = Math.min(startMs, endMs);
    const end = Math.max(startMs, endMs);
    this.draftTimeStart.set(toDateTimeLocalInput(start));
    this.draftTimeEnd.set(toDateTimeLocalInput(end));
  }

  protected toggleDraftTextAttributeValue(value: string, event: Event): void {
    this.draftTextAttributeValues.set(
      toggleSelection(
        this.draftTextAttributeValues(),
        value,
        (event.target as HTMLInputElement).checked,
      ),
    );
  }

  protected selectAllDraftEventTypes(): void {
    this.draftEventTypes.set([...this.filterOptions().event_types]);
  }

  protected clearDraftEventTypes(): void {
    this.draftEventTypes.set([]);
  }

  protected selectAllDraftObjectTypes(): void {
    this.draftObjectTypes.set([...this.filterOptions().object_types]);
  }

  protected clearDraftObjectTypes(): void {
    this.draftObjectTypes.set([]);
  }

  protected selectAllDraftDfNodes(): void {
    this.draftDfNodes.set([...this.filterOptions().event_types]);
  }

  protected clearDraftDfNodes(): void {
    this.draftDfNodes.set([]);
  }

  protected selectAllDraftDfEdges(): void {
    this.draftDfEdges.set(this.dfEdgeOptions().map(({ source, target }) => ({ source, target })));
  }

  protected clearDraftDfEdges(): void {
    this.draftDfEdges.set([]);
  }

  protected selectAllDraftTextAttributeValues(): void {
    this.draftTextAttributeValues.set([...(this.draftTextAttributeOption()?.values ?? [])]);
  }

  protected clearDraftTextAttributeValues(): void {
    this.draftTextAttributeValues.set([]);
  }

  protected draftTextAttributeOption(): TextAttributeOption | null {
    const key = this.draftTextAttributeKey();
    return (
      this.filterOptions().text_attributes.find((option) => textAttributeKey(option) === key) ??
      null
    );
  }

  private defaultTextAttributeFilter(): TextAttributeFilterRequest | null {
    const options = this.filterOptions().text_attributes;
    const option =
      options.find((candidate) => candidate.name === 'state' && candidate.scope === 'event') ??
      options[0];
    if (!option) {
      return null;
    }

    return {
      scope: option.scope,
      name: option.name,
      values: [],
    };
  }

  protected isDraftDfEdgeSelected(edge: DfEdgeFilterRequest): boolean {
    return this.draftDfEdges().some((candidate) => sameEdge(candidate, edge));
  }

  protected applyFilterDialog(): void {
    const dialog = this.filterDialog();

    if (dialog === 'activities') {
      this.selectedEventTypes.set([...this.draftEventTypes()]);
    }
    if (dialog === 'objectTypes') {
      this.selectedObjectTypes.set([...this.draftObjectTypes()]);
    }
    if (dialog === 'dfNodes') {
      this.selectedDfNodes.set([...this.draftDfNodes()]);
    }
    if (dialog === 'dfEdges') {
      this.selectedDfEdges.set([...this.draftDfEdges()]);
    }
    if (dialog === 'textAttributes') {
      const option = this.draftTextAttributeOption();
      const values = this.draftTextAttributeValues();
      this.selectedTextAttribute.set(
        option && values.length > 0
          ? {
              scope: option.scope,
              name: option.name,
              values,
            }
          : null,
      );
    }
    if (dialog === 'timeframe') {
      const range = normalizeTimeRange(
        fromDateTimeLocalInput(this.draftTimeStart()),
        fromDateTimeLocalInput(this.draftTimeEnd()),
        this.filterOptions().time_min_ms,
        this.filterOptions().time_max_ms,
      );
      this.selectedTimeRange.set(range);
    }

    this.filterDialog.set(null);
    this.isFilterChainOpen.set(false);
    this.applyActiveFilter();
  }

  protected removeFilter(kind: FilterDialogKind): void {
    if (kind === 'activities') {
      this.selectedEventTypes.set([...this.filterOptions().event_types]);
      this.draftEventTypes.set([...this.filterOptions().event_types]);
    } else if (kind === 'objectTypes') {
      this.selectedObjectTypes.set([...this.filterOptions().object_types]);
      this.draftObjectTypes.set([...this.filterOptions().object_types]);
    } else if (kind === 'dfNodes') {
      this.selectedDfNodes.set([]);
      this.draftDfNodes.set([]);
    } else if (kind === 'dfEdges') {
      this.selectedDfEdges.set([]);
      this.draftDfEdges.set([]);
    } else if (kind === 'timeframe') {
      this.selectedTimeRange.set(null);
      this.draftTimeStart.set('');
      this.draftTimeEnd.set('');
    } else if (kind === 'textAttributes') {
      this.selectedTextAttribute.set(null);
      this.draftTextAttributeKey.set('');
      this.draftTextAttributeValues.set([]);
    } else {
      this.selectedPatternFilters.set([]);
    }

    this.filterDialog.set(null);
    this.graphFilterMenu.set(null);
    this.isFilterChainOpen.set(false);
    this.applyActiveFilter();
  }

  protected onStateDetectionObjectTypeChange(event: Event): void {
    this.stateDetectionObjectType.set((event.target as HTMLSelectElement).value);
    this.stateDetectionAnalysis.set(null);
  }

  protected onStateDetectionWindowSizeChange(event: Event): void {
    this.stateDetectionWindowSize.set(
      clampInteger((event.target as HTMLInputElement).value, 1, 30),
    );
    this.stateDetectionAnalysis.set(null);
  }

  protected onStateDetectionSomWidthChange(event: Event): void {
    this.stateDetectionSomWidth.set(clampInteger((event.target as HTMLInputElement).value, 2, 12));
    this.stateDetectionAnalysis.set(null);
  }

  protected onStateDetectionSomHeightChange(event: Event): void {
    this.stateDetectionSomHeight.set(clampInteger((event.target as HTMLInputElement).value, 2, 12));
    this.stateDetectionAnalysis.set(null);
  }

  protected onStateDetectionColorAttributeChange(event: Event): void {
    this.stateDetectionColorAttribute.set((event.target as HTMLSelectElement).value);
    this.stateDetectionAnalysis.set(null);
  }

  protected runStateDetection(): void {
    this.loadStateDetection();
  }

  protected applyStateDetection(): void {
    if (!this.documentHandle || !this.stateDetectionAnalysis()) {
      return;
    }

    try {
      this.ensureStateDetectionObjectType();
      const result = JSON.parse(
        this.documentHandle.applyStateDetection(this.stateDetectionRequestJson()),
      ) as StateQueryResult;
      this.filterOptions.set(
        JSON.parse(this.documentHandle.filterOptionsJson()) as OcelFilterOptions,
      );
      this.summary.set(JSON.parse(this.documentHandle.summaryJson()) as OcelSummary);
      this.originalSummary.set(
        JSON.parse(this.documentHandle.originalSummaryJson()) as OcelSummary,
      );
      this.stateCorrelation.set(null);
      this.timePerspective.set(null);
      this.loadStatePatterns();
      this.loadStateAwareOcdfg();
      this.stateDetectionCellDetail.set(null);
      this.activeFeature.set('patterns');
      this.stateMessage.set(
        `Added ${result.attribute} for ${result.leading_object_type} from ${this.stateDetectionSomWidth()} x ${this.stateDetectionSomHeight()} SOM windows to ${result.assigned_events.toLocaleString()} of ${result.total_events.toLocaleString()} events.`,
      );
      this.errorMessage.set('');
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  }

  protected downloadStateFeatureTable(): void {
    if (!this.documentHandle) {
      return;
    }

    try {
      this.ensureStateDetectionObjectType();
      const csv = this.documentHandle.stateFeatureTableCsv(this.stateDetectionRequestJson());
      this.downloadNamed(
        csv,
        'text/csv',
        `${exportBaseName(this.fileName())}-${safeFilePart(this.stateDetectionObjectType())}-features.csv`,
      );
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  }

  protected previewColumns(analysis: StateDetectionResult, limit = 10): string[] {
    return analysis.feature_columns.slice(0, limit);
  }

  protected previewRows(analysis: StateDetectionResult): StateDetectionPreviewRow[] {
    return analysis.table_preview.slice(0, 15);
  }

  protected previewValues(row: StateDetectionPreviewRow, limit = 10): number[] {
    return row.values.slice(0, limit);
  }

  protected hiddenFeatureColumnCount(analysis: StateDetectionResult, limit = 10): number {
    return Math.max(analysis.feature_columns.length - limit, 0);
  }

  protected somGridColumns(analysis: StateDetectionResult): string {
    return `repeat(${analysis.som_width}, minmax(0, 1fr))`;
  }

  protected somCellStyle(cell: StateDetectionSomCell): string {
    const lightness = Math.round(94 - cell.color_value * 44);
    return `hsl(186 58% ${lightness}%)`;
  }

  protected somCellTitle(cell: StateDetectionSomCell): string {
    const activity = cell.dominant_activity ? ` | ${cell.dominant_activity}` : '';
    return `${cell.label}: ${cell.count.toLocaleString()} windows | ${cell.color_label}${activity}`;
  }

  protected topSomTransitions(
    transitions: StateDetectionSomTransition[],
    limit = 12,
  ): StateDetectionSomTransition[] {
    return transitions.slice(0, limit);
  }

  protected transitionLabel(transition: StateDetectionSomTransition): string {
    return `S${transition.source_x + 1}-${transition.source_y + 1} -> S${transition.target_x + 1}-${transition.target_y + 1}`;
  }

  protected percent(value: number): string {
    return `${Math.round(value * 1000) / 10}%`;
  }

  protected reloadStateCorrelation(): void {
    this.loadStateCorrelation();
  }

  protected correlationRows(analysis: StateCorrelationResult): StateCorrelationRow[] {
    return analysis.rows;
  }

  protected formatCorrelation(value: number): string {
    const rounded = Math.round(value * 1000) / 1000;
    const formatted = rounded.toFixed(3);
    return rounded > 0 ? `+${formatted}` : formatted;
  }

  protected formatFeatureNumber(value: number): string {
    if (!Number.isFinite(value)) {
      return '0';
    }
    if (Math.abs(value - Math.round(value)) < 0.000_000_1) {
      return Math.round(value).toLocaleString();
    }
    return value.toLocaleString(undefined, {
      maximumFractionDigits: 3,
    });
  }

  protected strengthLabel(value: number): string {
    return `${Math.round(value * 100)}%`;
  }

  protected correlationCellStyle(row: StateCorrelationRow): string {
    return correlationHeatStyle(row.correlation);
  }

  protected reloadTimePerspective(): void {
    this.loadTimePerspective();
  }

  protected onTimePerspectiveObjectTypeChange(event: Event): void {
    this.timePerspectiveObjectType.set((event.target as HTMLSelectElement).value);
    this.timePerspectiveFromState.set('');
    this.timePerspectiveToState.set('');
    this.loadTimePerspective();
  }

  protected onTimePerspectiveFromStateChange(event: Event): void {
    this.timePerspectiveFromState.set((event.target as HTMLSelectElement).value);
    this.loadTimePerspective();
  }

  protected onTimePerspectiveToStateChange(event: Event): void {
    this.timePerspectiveToState.set((event.target as HTMLSelectElement).value);
    this.loadTimePerspective();
  }

  protected onTimePerspectiveRoundtripChange(event: Event): void {
    this.timePerspectiveRoundtrip.set((event.target as HTMLInputElement).checked);
    this.loadTimePerspective();
  }

  protected performanceModeLabel(analysis: TimePerspectiveResult): string {
    return analysis.performance.roundtrip
      ? `${analysis.performance.from_state} -> ${analysis.performance.to_state} -> ${analysis.performance.from_state}`
      : `${analysis.performance.from_state} -> ${analysis.performance.to_state}`;
  }

  protected durationLabel(durationMs?: number | null): string {
    return formatDuration(durationMs);
  }

  protected timeLabel(timeMs: number): string {
    return formatDateTime(timeMs);
  }

  protected stateDistributionLabel(analysis: StateCorrelationResult): string {
    return analysis.state_distribution
      .map((entry) => `${entry.state}: ${entry.count.toLocaleString()}`)
      .join(', ');
  }

  protected openStateDetectionCell(cell: StateDetectionSomCell): void {
    if (!this.documentHandle || !this.stateDetectionAnalysis()) {
      return;
    }

    try {
      const request = {
        ...JSON.parse(this.stateDetectionRequestJson()),
        cell_x: cell.x,
        cell_y: cell.y,
      };
      const detail = JSON.parse(
        this.documentHandle.stateDetectionCellJson(JSON.stringify(request)),
      ) as StateDetectionCellDetail;
      this.stateDetectionCellDetail.set(detail);
      this.stateDetectionCellTab.set('dfg');
      this.errorMessage.set('');
    } catch (error) {
      this.stateDetectionCellDetail.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  }

  protected closeStateDetectionCell(): void {
    this.stateDetectionCellDetail.set(null);
    this.stateDetectionCellTab.set('dfg');
  }

  protected setStateDetectionCellTab(tab: StateDetectionCellTab): void {
    this.stateDetectionCellTab.set(tab);
  }

  protected stateDetectionCellGraphSettings(): ProcessGraphSettings {
    const objectType =
      this.stateDetectionAnalysis()?.object_type ?? this.stateDetectionObjectType();
    return {
      object_types: objectType ? [objectType] : [],
      min_activity_frequency: 1,
      min_path_frequency: 1,
    };
  }

  protected stateDetectionCellObjectTypes(): string[] {
    const objectType =
      this.stateDetectionAnalysis()?.object_type ?? this.stateDetectionObjectType();
    return objectType ? [objectType] : [];
  }

  protected ignoreStateDetectionCellGraphSettings(_settings: ProcessGraphSettings): void {
    return;
  }

  protected onCausalObjectTypeChange(event: Event): void {
    this.causalObjectType.set((event.target as HTMLSelectElement).value);
    this.resetCausalModel();
    this.loadCausalFeatureTable();
  }

  protected reloadCausalFeatureTable(): void {
    this.loadCausalFeatureTable();
  }

  protected downloadCausalFeatureTable(): void {
    if (!this.documentHandle) {
      return;
    }

    try {
      this.ensureCausalObjectType();
      if (!this.causalObjectType()) {
        return;
      }
      const csv = this.documentHandle.causalFeatureTableCsv(this.causalFeatureTableRequestJson());
      this.downloadNamed(
        csv,
        'text/csv',
        `${exportBaseName(this.fileName())}-${safeFilePart(this.causalObjectType())}-causal-features.csv`,
      );
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  }

  protected causalPreviewColumns(table: CausalFeatureTableResult): string[] {
    return table.feature_columns;
  }

  protected causalPreviewRows(table: CausalFeatureTableResult): StateDetectionPreviewRow[] {
    return table.table_preview.slice(0, 10);
  }

  protected causalPreviewValues(row: StateDetectionPreviewRow): number[] {
    return row.values;
  }

  protected onCausalFeatureDragStart(event: DragEvent, feature: string): void {
    event.dataTransfer?.setData('text/plain', feature);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'copy';
    }
  }

  protected allowCausalDrop(event: DragEvent): void {
    event.preventDefault();
  }

  protected dropCausalFeature(event: DragEvent, role: 'observable' | 'outcome'): void {
    event.preventDefault();
    const feature = event.dataTransfer?.getData('text/plain') ?? '';
    if (!feature) {
      return;
    }
    this.addCausalFeatureNode(role, feature);
  }

  protected addCausalFeatureNode(role: 'observable' | 'outcome', feature: string): void {
    const node: CausalModelNode = {
      id: nextCausalNodeId(role, this.causalNodes()),
      label: causalFeatureLabel(feature),
      role,
      feature,
      operation: 'identity',
    };
    this.causalNodes.set([...this.causalNodes(), node]);
    this.causalFit.set(null);
    this.causalMessage.set(`${role === 'observable' ? 'Observable' : 'Outcome'} added.`);
  }

  protected onCausalNodeLabelChange(nodeId: string, event: Event): void {
    const label = (event.target as HTMLInputElement).value;
    this.causalNodes.set(
      this.causalNodes().map((node) => (node.id === nodeId ? { ...node, label } : node)),
    );
    this.causalFit.set(null);
  }

  protected onCausalNodeRoleChange(nodeId: string, event: Event): void {
    const role = (event.target as HTMLSelectElement).value as 'observable' | 'outcome';
    const nodes = this.causalNodes().map((node) =>
      node.id === nodeId && node.role !== 'latent' ? { ...node, role } : node,
    );
    this.causalNodes.set(nodes);
    this.causalEdges.set(pruneCausalEdges(nodes, this.causalEdges()));
    this.causalFit.set(null);
  }

  protected onCausalOperationChange(nodeId: string, event: Event): void {
    const operation = (event.target as HTMLSelectElement).value as CausalOperation;
    this.causalNodes.set(
      this.causalNodes().map((node) => (node.id === nodeId ? { ...node, operation } : node)),
    );
    this.causalFit.set(null);
  }

  protected onCausalLatentDraftChange(event: Event): void {
    this.causalLatentDraft.set((event.target as HTMLInputElement).value);
  }

  protected addCausalLatent(): void {
    const label =
      this.causalLatentDraft().trim() || `Latent ${this.causalLatentNodes().length + 1}`;
    const node: CausalModelNode = {
      id: nextCausalNodeId('latent', this.causalNodes()),
      label,
      role: 'latent',
      operation: 'identity',
    };
    this.causalNodes.set([...this.causalNodes(), node]);
    this.causalLatentDraft.set('');
    this.causalFit.set(null);
    this.causalMessage.set('Latent variable added.');
  }

  protected removeCausalNode(nodeId: string): void {
    this.causalNodes.set(this.causalNodes().filter((node) => node.id !== nodeId));
    this.causalEdges.set(
      this.causalEdges().filter((edge) => edge.source !== nodeId && edge.target !== nodeId),
    );
    this.causalFit.set(null);
  }

  protected isCausalEdgeSelected(source: string, target: string): boolean {
    return this.causalEdges().some((edge) => edge.source === source && edge.target === target);
  }

  protected isCausalEdgeDisabled(source: string, target: string): boolean {
    if (this.isCausalEdgeSelected(source, target)) {
      return false;
    }
    return !canAddCausalEdge(this.causalNodes(), this.causalEdges(), source, target);
  }

  protected toggleCausalEdge(source: string, target: string, event: Event): void {
    const checked = (event.target as HTMLInputElement).checked;
    if (!checked) {
      this.causalEdges.set(
        this.causalEdges().filter((edge) => edge.source !== source || edge.target !== target),
      );
      this.causalFit.set(null);
      return;
    }

    if (!canAddCausalEdge(this.causalNodes(), this.causalEdges(), source, target)) {
      (event.target as HTMLInputElement).checked = false;
      this.causalMessage.set('That edge would violate the DAG or role constraints.');
      return;
    }

    this.causalEdges.set([...this.causalEdges(), { source, target }]);
    this.causalFit.set(null);
  }

  protected fitCausalModel(): void {
    if (!this.documentHandle || !this.canFitCausalModel()) {
      return;
    }

    try {
      const fit = JSON.parse(
        this.documentHandle.fitCausalModelJson(this.causalModelRequestJson()),
      ) as CausalFitResult;
      this.causalFit.set(fit);
      this.causalMessage.set(
        `Fitted ${fit.edges.length.toLocaleString()} edges over ${fit.sample_count.toLocaleString()} objects.`,
      );
      this.errorMessage.set('');
    } catch (error) {
      this.causalFit.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  }

  protected async generateCausalModelWithLlm(): Promise<void> {
    if (!this.documentHandle) {
      return;
    }

    const config = this.currentLlmConfig();
    if (!config.apiKey.trim()) {
      this.errorMessage.set('Configure and save an LLM API key first.');
      this.openLlmConfig();
      return;
    }

    if (!this.causalFeatureTable()) {
      this.loadCausalFeatureTable();
    }
    const table = this.causalFeatureTable();
    if (!table || table.feature_columns.length === 0) {
      this.errorMessage.set('Load causal features before asking the LLM.');
      return;
    }

    this.errorMessage.set('');
    this.causalMessage.set('');
    this.isGeneratingCausalModel.set(true);
    try {
      const response = await requestChatCompletion(config, [
        {
          role: 'system',
          content:
            'You generate Flowvault causal model JSON. Return only valid JSON and no markdown.',
        },
        {
          role: 'user',
          content: this.buildLlmCausalPrompt(table),
        },
      ]);
      const suggestion = parseCausalModelSuggestion(response, table.feature_columns);
      this.causalNodes.set(suggestion.nodes);
      this.causalEdges.set(pruneCausalEdges(suggestion.nodes, suggestion.edges));
      this.causalFit.set(null);
      this.causalMessage.set(
        `LLM suggested ${suggestion.nodes.length.toLocaleString()} nodes and ${this.causalEdges().length.toLocaleString()} DAG edges.`,
      );
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    } finally {
      this.isGeneratingCausalModel.set(false);
    }
  }

  protected summaryDisplayValue(metric: SummaryMetric): SummaryDisplayValue {
    const summary = this.summary();
    const originalSummary = this.originalSummary();
    const current = summary?.[metric] ?? 0;
    const original = originalSummary?.[metric] ?? current;

    if (!this.isFilterApplied()) {
      return {
        current: current.toLocaleString(),
        filtered: false,
      };
    }

    return {
      current: current.toLocaleString(),
      original: original.toLocaleString(),
      filtered: true,
    };
  }

  protected setIntraVisualization(visualization: PatternVisualization): void {
    this.intraVisualization.set(visualization);
  }

  protected setInterVisualization(visualization: PatternVisualization): void {
    this.interVisualization.set(visualization);
  }

  protected setPatternTab(tab: PatternTab): void {
    this.activePatternTab.set(tab);
  }

  protected togglePatternExplorer(): void {
    this.isPatternExplorerOpen.update((isOpen) => !isOpen);
  }

  protected openFullScreenGraph(pattern: StatePattern): void {
    this.fullScreenPattern.set(pattern);
  }

  protected closeFullScreenGraph(): void {
    this.fullScreenPattern.set(null);
  }

  protected applyPatternFilter(pattern: StatePattern): void {
    this.selectedPatternFilters.set([patternFilterRequest(pattern)]);
    this.applyActiveFilter();
  }

  protected openGraphNodeFilterMenu(event: ProcessGraphNodeFilterEvent): void {
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set({
      kind: 'node',
      activity: event.activity,
      ...graphMenuPosition(event.clientX, event.clientY),
    });
  }

  protected openGraphEdgeFilterMenu(event: ProcessGraphEdgeFilterEvent): void {
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set({
      kind: 'edge',
      source: event.source,
      target: event.target,
      ...graphMenuPosition(event.clientX, event.clientY),
    });
  }

  protected closeGraphFilterMenu(): void {
    this.graphFilterMenu.set(null);
  }

  protected applyGraphNodeFilter(activity: string): void {
    this.selectedDfNodes.set([...new Set([...this.selectedDfNodes(), activity])]);
    this.graphFilterMenu.set(null);
    this.applyActiveFilter();
  }

  protected applyGraphEdgeFilter(edge: DfEdgeFilterRequest): void {
    this.selectedDfEdges.set([...this.selectedDfEdges(), edge].filter(uniqueEdges));
    this.graphFilterMenu.set(null);
    this.applyActiveFilter();
  }

  protected applyStateAwareGraphSettings(settings: ProcessGraphSettings): void {
    this.stateAwareOcdfgSettings.set(this.sanitizeGraphSettings(settings));
    this.loadStateAwareOcdfg();
  }

  protected applyTraditionalGraphSettings(settings: ProcessGraphSettings): void {
    this.traditionalOcdfgSettings.set(this.sanitizeGraphSettings(settings));
    this.loadTraditionalOcdfg();
  }

  protected patternOptionLabel(pattern: StatePattern): string {
    return `${pattern.support.toLocaleString()}x | ${pattern.label}`;
  }

  protected patternFamilyLabel(pattern: StatePattern): string {
    return pattern.family === 'inter' ? 'Inter-state' : 'Intra-state';
  }

  protected patternStateLabel(pattern: StatePattern): string {
    return pattern.family === 'inter'
      ? `${pattern.from_state ?? '?'} -> ${pattern.to_state ?? '?'}`
      : (pattern.state ?? '?');
  }

  protected topEdges(edges: StatePatternEdge[], limit = 12): StatePatternEdge[] {
    return [...edges]
      .sort((left, right) => right.weight - left.weight || left.source.localeCompare(right.source))
      .slice(0, limit);
  }

  protected hiddenEdgeCount(edges: StatePatternEdge[], limit = 12): number {
    return Math.max(edges.length - limit, 0);
  }

  protected patternGraph(pattern: StatePattern, mode: boolean | 'compact' = false): PatternGraph {
    const expanded = mode === true;
    const compact = mode === 'compact';
    const nodeWidth = compact ? 150 : expanded ? 260 : 190;
    const nodeHeight = compact ? 54 : expanded ? 92 : 68;
    const controlGap = compact ? 188 : expanded ? 330 : 238;
    const controlStartX = compact ? 52 : expanded ? 120 : 86;
    const objectStartY = compact ? 186 : expanded ? 380 : 292;
    const objectColumnGap = compact ? 188 : expanded ? 330 : 236;
    const objectRowGap = compact ? 82 : expanded ? 140 : 104;
    const width = Math.max(
      compact ? 620 : expanded ? 1320 : 960,
      controlStartX * 2 + Math.max(pattern.sequence.length - 1, 0) * controlGap + nodeWidth,
    );
    const objectColumns = Math.max(1, Math.floor((width - 120) / objectColumnGap));
    const objectRows = Math.max(1, Math.ceil(pattern.object_types.length / objectColumns));
    const height = objectStartY + objectRows * objectRowGap + 54;

    const controlNodes = pattern.sequence.map((label, index) => ({
      id: `control-${index}`,
      lines: wrapGraphLabel(
        label,
        compact ? 17 : expanded ? 31 : 22,
        compact ? 3 : expanded ? 5 : 4,
      ),
      title: label,
      x: controlStartX + index * controlGap,
      y: 52,
      kind: label.startsWith('CHANGE ') ? ('change' as const) : ('control' as const),
    }));
    const objectNodes = pattern.object_types.map((objectType, index) => ({
      id: `object-${index}`,
      lines: wrapGraphLabel(
        objectType,
        compact ? 17 : expanded ? 31 : 22,
        compact ? 3 : expanded ? 5 : 4,
      ),
      title: objectType,
      x: controlStartX + (index % objectColumns) * objectColumnGap,
      y: objectStartY + Math.floor(index / objectColumns) * objectRowGap,
      kind: 'object' as const,
    }));
    const nodes = [...controlNodes, ...objectNodes];
    const firstControlByLabel = new Map<string, PatternGraphNode>();
    const objectByType = new Map<string, PatternGraphNode>();

    for (const [index, node] of controlNodes.entries()) {
      firstControlByLabel.set(pattern.sequence[index], node);
    }
    for (const [index, objectType] of pattern.object_types.entries()) {
      objectByType.set(objectType, objectNodes[index]);
    }

    const edges: PatternGraphEdge[] = [];
    for (let index = 0; index < controlNodes.length - 1; index += 1) {
      const source = controlNodes[index];
      const target = controlNodes[index + 1];
      const weight =
        pattern.df_edges.find(
          (edge) =>
            edge.source === pattern.sequence[index] && edge.target === pattern.sequence[index + 1],
        )?.weight ?? 1;
      edges.push({
        id: `df-${index}`,
        x1: source.x + nodeWidth,
        y1: source.y + nodeHeight / 2,
        x2: target.x,
        y2: target.y + nodeHeight / 2,
        label: weight.toLocaleString(),
        kind: 'df',
      });
    }

    for (const [index, edge] of pattern.eo_edges.entries()) {
      const source = firstControlByLabel.get(edge.source);
      const target = objectByType.get(edge.target);
      if (!source || !target) {
        continue;
      }
      edges.push({
        id: `eo-${index}`,
        x1: source.x + nodeWidth / 2,
        y1: source.y + nodeHeight,
        x2: target.x + nodeWidth / 2,
        y2: target.y,
        label: edge.weight.toLocaleString(),
        kind: 'eo',
      });
    }

    for (const [index, edge] of pattern.oo_edges.entries()) {
      const source = objectByType.get(edge.source);
      const target = objectByType.get(edge.target);
      if (!source || !target || source === target) {
        continue;
      }
      edges.push({
        id: `oo-${index}`,
        x1: source.x + nodeWidth / 2,
        y1: source.y + nodeHeight / 2,
        x2: target.x + nodeWidth / 2,
        y2: target.y + nodeHeight / 2,
        label: edge.weight.toLocaleString(),
        kind: 'oo',
      });
    }

    return { width, height, nodeWidth, nodeHeight, nodes, edges };
  }

  private loadCausalFeatureTable(): void {
    if (!this.documentHandle) {
      this.causalFeatureTable.set(null);
      return;
    }

    try {
      this.ensureCausalObjectType();
      if (!this.causalObjectType()) {
        this.causalFeatureTable.set(null);
        return;
      }
      const table = JSON.parse(
        this.documentHandle.causalFeatureTableJson(this.causalFeatureTableRequestJson()),
      ) as CausalFeatureTableResult;
      this.causalFeatureTable.set(table);
      this.causalNodes.set(
        this.causalNodes().filter(
          (node) => node.role === 'latent' || table.feature_columns.includes(node.feature ?? ''),
        ),
      );
      this.causalEdges.set(pruneCausalEdges(this.causalNodes(), this.causalEdges()));
      this.causalFit.set(null);
      this.causalMessage.set(
        `Loaded ${table.feature_count.toLocaleString()} features for ${table.object_count.toLocaleString()} objects.`,
      );
      this.errorMessage.set('');
    } catch (error) {
      this.causalFeatureTable.set(null);
      this.causalFit.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  }

  private ensureCausalObjectType(): void {
    const selected = this.selectedObjectTypes();
    const current = this.causalObjectType();
    if (current && selected.includes(current)) {
      return;
    }
    this.causalObjectType.set(selected[0] ?? this.filterOptions().object_types[0] ?? '');
  }

  private resetCausalModel(): void {
    this.causalFeatureTable.set(null);
    this.causalNodes.set([]);
    this.causalEdges.set([]);
    this.causalLatentDraft.set('');
    this.causalFit.set(null);
    this.causalMessage.set('');
  }

  private causalFeatureTableRequestJson(): string {
    return JSON.stringify({ object_type: this.causalObjectType() });
  }

  private causalModelRequestJson(): string {
    return JSON.stringify({
      object_type: this.causalObjectType(),
      nodes: this.causalNodes(),
      edges: this.causalEdges(),
    });
  }

  private loadStateDetection(): void {
    if (!this.documentHandle) {
      this.stateDetectionAnalysis.set(null);
      return;
    }

    try {
      this.ensureStateDetectionObjectType();
      if (!this.stateDetectionObjectType()) {
        this.stateDetectionAnalysis.set(null);
        return;
      }
      const analysis = JSON.parse(
        this.documentHandle.stateDetectionJson(this.stateDetectionRequestJson()),
      ) as StateDetectionResult;
      this.stateDetectionAnalysis.set(analysis);
      this.stateDetectionColorOptions.set(analysis.color_attributes);
      this.stateDetectionColorAttribute.set(analysis.color_attribute);
      this.stateDetectionCellDetail.set(null);
      this.errorMessage.set('');
    } catch (error) {
      this.stateDetectionAnalysis.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  }

  private stateDetectionRequestJson(): string {
    return JSON.stringify({
      object_type: this.stateDetectionObjectType(),
      window_size: this.stateDetectionWindowSize(),
      som_width: this.stateDetectionSomWidth(),
      som_height: this.stateDetectionSomHeight(),
      color_attribute: this.stateDetectionColorAttribute(),
    });
  }

  private ensureStateDetectionObjectType(): void {
    const selected = this.selectedObjectTypes();
    const current = this.stateDetectionObjectType();
    if (current && selected.includes(current)) {
      return;
    }
    this.stateDetectionObjectType.set(selected[0] ?? this.filterOptions().object_types[0] ?? '');
  }

  private currentLlmConfig(): LlmConfig {
    const provider = providerById(this.llmProvider());
    return {
      provider: provider.id,
      model: this.llmModel().trim() || provider.defaultModel,
      apiKey: this.llmApiKey().trim(),
    };
  }

  private persistStateExpression(expression: string): void {
    const normalized = expression.trim();
    if (!normalized) {
      return;
    }

    this.persistedStateExpression.set(normalized);
    writeStoredString(STATE_EXPRESSION_STORAGE_KEY, normalized);
  }

  private buildLlmStatePrompt(): string {
    const summary = this.summary();
    const originalSummary = this.originalSummary();
    const stateDetection = this.stateDetectionAnalysis();
    const examples = STATE_EXPRESSION_EXAMPLES.map((example, index) => {
      return `Example ${index + 1}:\n${example}`;
    }).join('\n\n');
    const stateDetectionMetadata = stateDetection
      ? `
State Detection feature columns for ${stateDetection.object_type}:
${stateDetection.feature_columns
  .slice(0, 30)
  .map((column) => `- ${column}`)
  .join('\n')}
`
      : '';

    return `${this.llmStatePrompt().trim() || DEFAULT_LLM_STATE_PROMPT}

Basic OCEL metadata:
- File: ${this.fileName() || 'unknown'}
- Active events: ${summary?.events ?? 0}
- Active objects: ${summary?.objects ?? 0}
- Original events: ${originalSummary?.events ?? summary?.events ?? 0}
- Original objects: ${originalSummary?.objects ?? summary?.objects ?? 0}
- Event types: ${this.filterOptions().event_types.join(', ') || 'unknown'}
- Object types: ${this.filterOptions().object_types.join(', ') || 'unknown'}
- Active event types: ${this.selectedEventTypes().join(', ') || 'none'}
- Active object types: ${this.selectedObjectTypes().join(', ') || 'none'}
- Leading object type to use: ${this.selectedLeadingObjectType() || 'choose one from active object types'}
${stateDetectionMetadata}
State expression language:
- Shape: STATE state FOR LEADING OBJECT TYPE '<object type>' AS CASE ... END
- Each branch is WHEN <condition> THEN '<state label>'.
- Add ELSE '<state label>' unless a partial state assignment is intentional.
- Use event.type for the activity name.
- Use event.attribute_name or event."Attribute Name" for event attributes.
- Use object.attribute_name or object."Attribute Name" for the selected leading object's attributes.
- Supported comparisons include =, !=, <, <=, >, >=, LIKE, IS NULL, IS NOT NULL.
- Combine conditions with AND, OR, NOT and parentheses.
- Return concise, interpretable state labels.

Few-shot examples:
${examples}

Return only one valid Flowvault state expression.`;
  }

  private buildLlmCausalPrompt(table: CausalFeatureTableResult): string {
    const summary = this.summary();
    const features = table.feature_columns
      .slice(0, 160)
      .map((feature) => `- ${feature}`)
      .join('\n');
    const omitted = Math.max(table.feature_columns.length - 160, 0);

    return `Create a small DAG causal model for Flowvault.

OCEL metadata:
- File: ${this.fileName() || 'unknown'}
- Active events: ${summary?.events ?? 0}
- Active objects: ${summary?.objects ?? 0}
- Causal object type: ${table.object_type}
- Rows in feature table: ${table.object_count}
- Feature columns: ${table.feature_count}

Available feature columns:
${features}
${omitted > 0 ? `- ... ${omitted} additional columns omitted from the prompt` : ''}

Causal model JSON schema:
{
  "nodes": [
    {"id":"obs_1","label":"Readable name","role":"observable","feature":"exact feature column","operation":"identity"},
    {"id":"lat_1","label":"Latent concept","role":"latent"},
    {"id":"out_1","label":"Readable outcome","role":"outcome","feature":"exact feature column","operation":"identity"}
  ],
  "edges": [
    {"source":"obs_1","target":"lat_1"},
    {"source":"lat_1","target":"out_1"}
  ]
}

Rules:
- Return only JSON.
- Use only exact feature column names from the list.
- Valid roles are observable, latent, outcome.
- Valid operations are identity, log10, log_e, sqrt.
- Legal edges are observable -> latent, latent -> latent, and latent -> outcome.
- The graph must be a DAG.
- The same feature column may appear in multiple observable/outcome nodes.
- Prefer 2-5 observables, 1-3 latents, and 1-3 outcomes.`;
  }

  private loadStatePatterns(preserveSelection = false): void {
    if (!this.documentHandle) {
      this.patternAnalysis.set(null);
      this.stateAwareOcdfg.set(null);
      return;
    }

    const previousIntraId = this.selectedIntraPatternId();
    const previousInterId = this.selectedInterPatternId();
    const previousFullScreenPatternId = this.fullScreenPattern()?.id;
    const analysis = JSON.parse(this.documentHandle.statePatternsJson()) as StatePatternAnalysis;
    this.patternAnalysis.set(analysis);
    this.loadStateAwareOcdfg();
    if (!preserveSelection) {
      this.activePatternTab.set('intra');
    }
    this.selectedIntraPatternId.set(
      preserveSelection
        ? (selectedPattern(analysis.intra, previousIntraId)?.id ?? '')
        : (analysis.intra[0]?.id ?? ''),
    );
    this.selectedInterPatternId.set(
      preserveSelection
        ? (selectedPattern(analysis.inter, previousInterId)?.id ?? '')
        : (analysis.inter[0]?.id ?? ''),
    );

    if (previousFullScreenPatternId) {
      this.fullScreenPattern.set(
        [...analysis.intra, ...analysis.inter].find(
          (pattern) => pattern.id === previousFullScreenPatternId,
        ) ?? null,
      );
    }
  }

  private loadStateCorrelation(): void {
    if (!this.documentHandle) {
      this.stateCorrelation.set(null);
      return;
    }

    try {
      this.stateCorrelation.set(
        JSON.parse(this.documentHandle.stateCorrelationsJson()) as StateCorrelationResult,
      );
      this.errorMessage.set('');
    } catch (error) {
      this.stateCorrelation.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  }

  private loadTimePerspective(): void {
    if (!this.documentHandle) {
      this.timePerspective.set(null);
      return;
    }

    try {
      this.ensureTimePerspectiveObjectType();
      const analysis = JSON.parse(
        this.documentHandle.timePerspectiveJson(this.timePerspectiveRequestJson()),
      ) as TimePerspectiveResult;
      this.timePerspective.set(analysis);
      this.timePerspectiveObjectType.set(analysis.object_type);
      this.timePerspectiveFromState.set(
        validStateSelection(
          this.timePerspectiveFromState(),
          analysis.states,
          analysis.performance.from_state,
        ),
      );
      this.timePerspectiveToState.set(
        validStateSelection(
          this.timePerspectiveToState(),
          analysis.states,
          analysis.performance.to_state,
        ),
      );
      this.errorMessage.set('');
    } catch (error) {
      this.timePerspective.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  }

  private ensureTimePerspectiveObjectType(): void {
    const selected = this.selectedObjectTypes();
    const current = this.timePerspectiveObjectType();
    if (current && selected.includes(current)) {
      return;
    }
    this.timePerspectiveObjectType.set(selected[0] ?? this.filterOptions().object_types[0] ?? '');
  }

  private timePerspectiveRequestJson(): string {
    return JSON.stringify({
      object_type: this.timePerspectiveObjectType() || undefined,
      from_state: this.timePerspectiveFromState() || undefined,
      to_state: this.timePerspectiveToState() || undefined,
      roundtrip: this.timePerspectiveRoundtrip(),
      buckets: 32,
    });
  }

  private loadStateAwareOcdfg(): void {
    if (!this.documentHandle) {
      this.stateAwareOcdfg.set(null);
      return;
    }

    try {
      this.stateAwareOcdfg.set(
        JSON.parse(
          this.documentHandle.filteredStateAwareObjectCentricDirectlyFollowsGraphJson(
            graphRequestJson(this.stateAwareOcdfgSettings()),
          ),
        ) as ProcessGraph,
      );
    } catch (error) {
      this.stateAwareOcdfg.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  }

  private loadTraditionalOcdfg(): void {
    if (!this.documentHandle) {
      this.traditionalOcdfg.set(null);
      return;
    }

    try {
      this.traditionalOcdfg.set(
        JSON.parse(
          this.documentHandle.filteredObjectCentricDirectlyFollowsGraphJson(
            graphRequestJson(this.traditionalOcdfgSettings()),
          ),
        ) as ProcessGraph,
      );
    } catch (error) {
      this.traditionalOcdfg.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  }

  private applyActiveFilter(): void {
    if (!this.documentHandle) {
      return;
    }

    const filter: FilterRequest = {
      event_types: this.selectedEventTypes(),
      object_types: this.selectedObjectTypes(),
    };
    if (this.selectedDfNodes().length > 0) {
      filter.df_nodes = this.selectedDfNodes();
    }
    if (this.selectedDfEdges().length > 0) {
      filter.df_edges = this.selectedDfEdges();
    }
    const timeRange = this.selectedTimeRange();
    if (timeRange) {
      filter.time_range = timeRange;
    }
    const textAttribute = this.selectedTextAttribute();
    if (textAttribute && textAttribute.values.length > 0) {
      filter.text_attributes = [textAttribute];
    }
    if (this.selectedPatternFilters().length > 0) {
      filter.patterns = this.selectedPatternFilters();
    }

    try {
      const nextSummary = JSON.parse(
        this.documentHandle.applyFilter(JSON.stringify(filter)),
      ) as OcelSummary;

      this.summary.set(nextSummary);
      this.originalSummary.set(
        JSON.parse(this.documentHandle.originalSummaryJson()) as OcelSummary,
      );
      this.stateAwareOcdfgSettings.set(this.sanitizeGraphSettings(this.stateAwareOcdfgSettings()));
      this.traditionalOcdfgSettings.set(
        this.sanitizeGraphSettings(this.traditionalOcdfgSettings()),
      );
      this.loadTraditionalOcdfg();
      this.graphFilterMenu.set(null);
      this.updateStateMessageAfterFilter(nextSummary);
      this.ensureStateDetectionObjectType();
      this.stateDetectionCellDetail.set(null);
      if (this.activeFeature() === 'stateDetection' || this.stateDetectionAnalysis()) {
        this.loadStateDetection();
      }
      this.ensureCausalObjectType();
      if (this.activeFeature() === 'causalModel' || this.causalFeatureTable()) {
        this.loadCausalFeatureTable();
      }

      this.stateCorrelation.set(null);
      this.timePerspective.set(null);
      if (nextSummary.stateful_events > 0) {
        this.loadStatePatterns(true);
        if (this.activeFeature() === 'correlation') {
          this.loadStateCorrelation();
        }
        if (this.activeFeature() === 'timePerspective') {
          this.loadTimePerspective();
        }
      } else {
        this.patternAnalysis.set(null);
        this.stateCorrelation.set(null);
        this.timePerspective.set(null);
        this.stateAwareOcdfg.set(null);
        this.selectedIntraPatternId.set('');
        this.selectedInterPatternId.set('');
        this.activePatternTab.set('intra');
        this.fullScreenPattern.set(null);
      }
      this.errorMessage.set('');
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  }

  private updateStateMessageAfterFilter(summary: OcelSummary): void {
    const originalSummary = this.originalSummary();

    if (!originalSummary?.stateful_events) {
      this.stateMessage.set('');
      return;
    }

    if (summary.stateful_events > 0) {
      this.stateMessage.set(
        `State retained on ${summary.stateful_events.toLocaleString()} of ${summary.events.toLocaleString()} active events.`,
      );
      return;
    }

    this.stateMessage.set('State is retained in the original log, but no active events match it.');
  }

  private resetGraphSettings(objectTypes: string[]): void {
    const settings = {
      object_types: [...objectTypes],
      min_activity_frequency: 1,
      min_path_frequency: 1,
    };
    this.stateAwareOcdfgSettings.set(settings);
    this.traditionalOcdfgSettings.set(settings);
  }

  private sanitizeGraphSettings(settings: ProcessGraphSettings): ProcessGraphSettings {
    const availableObjectTypes = new Set(this.selectedObjectTypes());
    const objectTypes = settings.object_types.filter((objectType) =>
      availableObjectTypes.has(objectType),
    );

    return {
      object_types: objectTypes,
      min_activity_frequency: Math.max(1, Math.round(settings.min_activity_frequency || 1)),
      min_path_frequency: Math.max(1, Math.round(settings.min_path_frequency || 1)),
    };
  }
}

function emptyGraphSettings(): ProcessGraphSettings {
  return {
    object_types: [],
    min_activity_frequency: 1,
    min_path_frequency: 1,
  };
}

function correlationHeatStyle(correlation: number): string {
  const value = Number.isFinite(correlation) ? Math.max(-1, Math.min(1, correlation)) : 0;
  const strength = Math.abs(value);
  if (strength < 0.05) {
    return 'background: #f4f7f6; color: #263632;';
  }

  const hue = value >= 0 ? 166 : 22;
  const saturation = value >= 0 ? 48 : 68;
  const lightness = Math.round(96 - strength * 45);
  const color = strength >= 0.68 ? '#ffffff' : '#17221f';
  return `background: hsl(${hue} ${saturation}% ${lightness}%); color: ${color};`;
}

const LLM_CONFIG_STORAGE_KEY = 'flowvault.llmConfig';
const STATE_EXPRESSION_STORAGE_KEY = 'flowvault.stateExpression';
const DEFAULT_LLM_STATE_PROMPT = 'Give me a state expression for this object-centric event log.';
const STATE_EXPRESSION_EXAMPLES = [
  `STATE state FOR LEADING OBJECT TYPE 'Invoice' AS CASE
  WHEN object.is_blocked = 'Yes' THEN 'Invoice Blocked'
  WHEN event.type LIKE '%Payment%' THEN 'Payment Execution'
  WHEN event.type LIKE '%Invoice%' THEN 'Invoice Handling'
  ELSE 'Procurement'
END`,
  `STATE state FOR LEADING OBJECT TYPE 'MAT' AS CASE
  WHEN event."Stock After" = 0 THEN 'Zero Stock'
  WHEN event."Stock After" < 30 THEN 'Low Stock'
  WHEN event."Stock After" >= 100 THEN 'High Stock'
  ELSE 'Available Stock'
END`,
  `STATE state FOR LEADING OBJECT TYPE 'orders' AS CASE
  WHEN event.type = 'item out of stock' THEN 'Stock Exception'
  WHEN event.type = 'reorder item' THEN 'Replenishment'
  WHEN event.type = 'payment reminder' THEN 'Payment Risk'
  ELSE 'Nominal'
END`,
];

function loadLlmConfig(): LlmConfig {
  const stored = readStoredJson(LLM_CONFIG_STORAGE_KEY);
  if (!stored || typeof stored !== 'object') {
    return defaultLlmConfig();
  }

  const candidate = stored as Partial<LlmConfig>;
  const provider = providerById(String(candidate.provider ?? DEFAULT_LLM_PROVIDER.id));
  return {
    provider: provider.id,
    model: String(candidate.model ?? provider.defaultModel),
    apiKey: String(candidate.apiKey ?? ''),
  };
}

function defaultLlmConfig(): LlmConfig {
  return {
    provider: DEFAULT_LLM_PROVIDER.id,
    model: DEFAULT_LLM_PROVIDER.defaultModel,
    apiKey: '',
  };
}

function defaultStateQuery(leadingObjectType: string): string {
  return `STATE state FOR LEADING OBJECT TYPE '${leadingObjectType}' AS CASE
  WHEN event.type IS NOT NULL THEN 'Active'
  ELSE 'Other'
END`;
}

function extractStateExpression(response: string): string {
  const fenced = response.match(/```(?:sql|text)?\s*([\s\S]*?)```/i);
  const candidate = (fenced?.[1] ?? response).trim();
  const start = candidate.toUpperCase().indexOf('STATE ');
  if (start >= 0) {
    return candidate.slice(start).trim();
  }
  return candidate;
}

function readStoredString(key: string): string {
  try {
    return globalThis.localStorage?.getItem(key) ?? '';
  } catch {
    return '';
  }
}

function writeStoredString(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch {
    return;
  }
}

function readStoredJson(key: string): unknown {
  const stored = readStoredString(key);
  if (!stored) {
    return null;
  }

  try {
    return JSON.parse(stored);
  } catch {
    return null;
  }
}

function writeStoredJson(key: string, value: unknown): void {
  try {
    globalThis.localStorage?.setItem(key, JSON.stringify(value));
  } catch {
    return;
  }
}

const DEFAULT_STATE_DETECTION_COLOR_OPTIONS: StateDetectionColorOption[] = [
  {
    id: '__window_count',
    label: 'Assigned windows',
    kind: 'count',
  },
];

const STATIC_SAMPLE_LOGS: StaticSampleLog[] = [
  {
    label: 'Purchase-to-Pay JSON',
    detail: 'Small example log',
    fileName: 'ocel20_example.json.gz',
    path: 'static/ocel2_compressed/ocel20_example.json.gz',
  },
  {
    label: 'Purchase-to-Pay XML',
    detail: 'Small example log',
    fileName: 'ocel20_example.xml.gz',
    path: 'static/ocel2_compressed/ocel20_example.xml.gz',
  },
  {
    label: 'Order Management JSON',
    detail: 'Order, item, and delivery flow',
    fileName: 'order-management.json.gz',
    path: 'static/ocel2_compressed/order-management.json.gz',
  },
  {
    label: 'Order Management XML',
    detail: 'Order, item, and delivery flow',
    fileName: 'order-management.xml.gz',
    path: 'static/ocel2_compressed/order-management.xml.gz',
  },
  {
    label: 'Container Logistics JSON',
    detail: 'Shipment and warehouse flow',
    fileName: 'container_logistics.json.gz',
    path: 'static/ocel2_compressed/container_logistics.json.gz',
  },
  {
    label: 'Container Logistics XML',
    detail: 'Shipment and warehouse flow',
    fileName: 'container_logistics.xml.gz',
    path: 'static/ocel2_compressed/container_logistics.xml.gz',
  },
  {
    label: 'Inventory Simulation JSON',
    detail: 'Stock and replenishment flow',
    fileName: 'inventory_management_simulated.json.gz',
    path: 'static/ocel2_compressed/inventory_management_simulated.json.gz',
  },
  {
    label: 'Inventory Simulation XML',
    detail: 'Stock and replenishment flow',
    fileName: 'inventory_management_simulated.xml.gz',
    path: 'static/ocel2_compressed/inventory_management_simulated.xml.gz',
  },
];

function graphRequestJson(settings: ProcessGraphSettings): string {
  return JSON.stringify(settings);
}

function errorToMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === 'string') {
    return error;
  }

  return 'Could not process the OCEL file.';
}

function selectedPattern(patterns: StatePattern[], selectedId: string): StatePattern | null {
  return patterns.find((pattern) => pattern.id === selectedId) ?? patterns[0] ?? null;
}

function emptySummaryValue(): SummaryDisplayValue {
  return {
    current: '0',
    filtered: false,
  };
}

function toggleSelection(values: string[], value: string, checked: boolean): string[] {
  if (checked) {
    return values.includes(value) ? values : [...values, value];
  }

  return values.filter((candidate) => candidate !== value);
}

function filterDescription(prefix: string, values: string[]): string {
  return values.length > 0 ? `${prefix}: ${values.join(', ')}` : `${prefix}: none`;
}

function edgeLabel(edge: DfEdgeFilterRequest): string {
  return `${edge.source} -> ${edge.target}`;
}

function edgeKey(edge: DfEdgeFilterRequest): string {
  return `${edge.source}\u0000${edge.target}`;
}

function sameEdge(left: DfEdgeFilterRequest, right: DfEdgeFilterRequest): boolean {
  return left.source === right.source && left.target === right.target;
}

function uniqueEdges(
  edge: DfEdgeFilterRequest,
  index: number,
  edges: DfEdgeFilterRequest[],
): boolean {
  return edges.findIndex((candidate) => sameEdge(candidate, edge)) === index;
}

const CAUSAL_OPERATIONS: Array<{ id: CausalOperation; label: string }> = [
  { id: 'identity', label: 'Identity' },
  { id: 'log10', label: 'log_10' },
  { id: 'log_e', label: 'log_e' },
  { id: 'sqrt', label: 'sqrt' },
];

function nextCausalNodeId(role: CausalNodeRole, nodes: CausalModelNode[]): string {
  const prefix = role === 'observable' ? 'obs' : role === 'outcome' ? 'out' : 'lat';
  const existing = new Set(nodes.map((node) => node.id));
  for (let index = 1; ; index += 1) {
    const id = `${prefix}-${index}`;
    if (!existing.has(id)) {
      return id;
    }
  }
}

function parseCausalModelSuggestion(
  response: string,
  availableFeatures: string[],
): { nodes: CausalModelNode[]; edges: CausalModelEdge[] } {
  const parsed = readJsonObject(extractJsonPayload(response));
  const rawNodes = Array.isArray(parsed['nodes']) ? parsed['nodes'] : [];
  const rawEdges = Array.isArray(parsed['edges']) ? parsed['edges'] : [];
  const featureSet = new Set(availableFeatures);
  const nodes: CausalModelNode[] = [];
  const originalToNewId = new Map<string, string>();

  for (const rawNode of rawNodes) {
    if (!rawNode || typeof rawNode !== 'object') {
      continue;
    }
    const record = rawNode as Record<string, unknown>;
    const role = normalizeCausalRole(record['role']);
    if (!role) {
      continue;
    }
    const originalId = String(record['id'] ?? record['label'] ?? `${role}-${nodes.length + 1}`);
    const label = String(record['label'] ?? record['name'] ?? originalId).trim();
    if (role === 'latent') {
      const node: CausalModelNode = {
        id: nextCausalNodeId('latent', nodes),
        label: label || `Latent ${nodes.filter((node) => node.role === 'latent').length + 1}`,
        role,
        operation: 'identity',
      };
      nodes.push(node);
      originalToNewId.set(originalId, node.id);
      originalToNewId.set(node.label, node.id);
      continue;
    }

    const feature = String(record['feature'] ?? '').trim();
    if (!featureSet.has(feature)) {
      continue;
    }
    const operation = normalizeCausalOperation(record['operation']);
    const node: CausalModelNode = {
      id: nextCausalNodeId(role, nodes),
      label: label || causalFeatureLabel(feature),
      role,
      feature,
      operation,
    };
    nodes.push(node);
    originalToNewId.set(originalId, node.id);
    originalToNewId.set(node.label, node.id);
  }

  const edges: CausalModelEdge[] = [];
  for (const rawEdge of rawEdges) {
    if (!rawEdge || typeof rawEdge !== 'object') {
      continue;
    }
    const record = rawEdge as Record<string, unknown>;
    const source = originalToNewId.get(String(record['source'] ?? ''));
    const target = originalToNewId.get(String(record['target'] ?? ''));
    if (!source || !target || !canAddCausalEdge(nodes, edges, source, target)) {
      continue;
    }
    edges.push({ source, target });
  }

  if (nodes.length === 0) {
    throw new Error('The LLM did not return any usable causal model nodes.');
  }
  return { nodes, edges };
}

function extractJsonPayload(response: string): string {
  const fenced = response.match(/```(?:json)?\s*([\s\S]*?)```/i);
  const candidate = (fenced?.[1] ?? response).trim();
  const start = candidate.indexOf('{');
  const end = candidate.lastIndexOf('}');
  if (start >= 0 && end > start) {
    return candidate.slice(start, end + 1);
  }
  return candidate;
}

function readJsonObject(jsonText: string): Record<string, unknown> {
  const parsed = JSON.parse(jsonText) as unknown;
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('The LLM response was not a JSON object.');
  }
  return parsed as Record<string, unknown>;
}

function normalizeCausalRole(value: unknown): CausalNodeRole | null {
  const role = String(value ?? '')
    .trim()
    .toLowerCase();
  if (role === 'observable' || role === 'latent' || role === 'outcome') {
    return role;
  }
  return null;
}

function normalizeCausalOperation(value: unknown): CausalOperation {
  const operation = String(value ?? '')
    .trim()
    .toLowerCase();
  if (operation === 'log_10' || operation === 'log10') {
    return 'log10';
  }
  if (operation === 'ln' || operation === 'loge' || operation === 'log_e') {
    return 'log_e';
  }
  if (operation === 'sqrt' || operation === 'square_root') {
    return 'sqrt';
  }
  return 'identity';
}

function causalFeatureLabel(feature: string): string {
  return feature
    .replace(/^activity\./, '')
    .replace(/^attribute\./, '')
    .replace(/^related_objects\./, 'Related ')
    .replace(/=/g, ' = ');
}

function canAddCausalEdge(
  nodes: CausalModelNode[],
  edges: CausalModelEdge[],
  source: string,
  target: string,
): boolean {
  if (source === target || edges.some((edge) => edge.source === source && edge.target === target)) {
    return false;
  }
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const sourceNode = byId.get(source);
  const targetNode = byId.get(target);
  if (!sourceNode || !targetNode || !isLegalCausalEdge(sourceNode, targetNode)) {
    return false;
  }
  return !causalGraphHasCycle(nodes, [...edges, { source, target }]);
}

function isLegalCausalEdge(source: CausalModelNode, target: CausalModelNode): boolean {
  return (
    (source.role === 'observable' && target.role === 'latent') ||
    (source.role === 'latent' && target.role === 'latent') ||
    (source.role === 'latent' && target.role === 'outcome')
  );
}

function pruneCausalEdges(nodes: CausalModelNode[], edges: CausalModelEdge[]): CausalModelEdge[] {
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const pruned: CausalModelEdge[] = [];
  for (const edge of edges) {
    const source = byId.get(edge.source);
    const target = byId.get(edge.target);
    if (!source || !target || !isLegalCausalEdge(source, target)) {
      continue;
    }
    if (!causalGraphHasCycle(nodes, [...pruned, edge])) {
      pruned.push(edge);
    }
  }
  return pruned;
}

function causalGraphHasCycle(nodes: CausalModelNode[], edges: CausalModelEdge[]): boolean {
  const indegree = new Map(nodes.map((node) => [node.id, 0]));
  const outgoing = new Map(nodes.map((node) => [node.id, [] as string[]]));
  for (const edge of edges) {
    if (!indegree.has(edge.source) || !indegree.has(edge.target)) {
      continue;
    }
    indegree.set(edge.target, (indegree.get(edge.target) ?? 0) + 1);
    outgoing.get(edge.source)?.push(edge.target);
  }

  const ready = nodes.filter((node) => (indegree.get(node.id) ?? 0) === 0).map((node) => node.id);
  let visited = 0;
  while (ready.length > 0) {
    const nodeId = ready.shift() ?? '';
    visited += 1;
    for (const target of outgoing.get(nodeId) ?? []) {
      const next = (indegree.get(target) ?? 0) - 1;
      indegree.set(target, next);
      if (next === 0) {
        ready.push(target);
      }
    }
  }
  return visited !== nodes.length;
}

function causalFitGraph(fit: CausalFitResult): CausalFitGraph {
  const nodeWidth = 190;
  const nodeHeight = 74;
  const columns: CausalNodeRole[] = ['observable', 'latent', 'outcome'];
  const columnX: Record<CausalNodeRole, number> = {
    observable: 60,
    latent: 360,
    outcome: 660,
  };
  const nodes: CausalFitGraphNode[] = [];
  for (const role of columns) {
    const roleNodes = fit.nodes.filter((node) => node.role === role);
    roleNodes.forEach((node, index) => {
      nodes.push({
        id: node.id,
        label: node.label,
        role: node.role,
        x: columnX[role],
        y: 48 + index * 116,
        width: nodeWidth,
        height: nodeHeight,
        lines: wrapGraphLabel(node.label, 20, 3),
      });
    });
  }
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const maxRows = Math.max(
    1,
    ...columns.map((role) => fit.nodes.filter((node) => node.role === role).length),
  );
  const width = 900;
  const height = 70 + maxRows * 116;
  const edges = fit.edges
    .map((edge, index) => {
      const source = byId.get(edge.source);
      const target = byId.get(edge.target);
      if (!source || !target) {
        return null;
      }
      const x1 = source.x + source.width;
      const y1 = source.y + source.height / 2;
      const x2 = target.x;
      const y2 = target.y + target.height / 2;
      const midX = (x1 + x2) / 2;
      return {
        id: `causal-edge-${index}`,
        source,
        target,
        edge,
        path: `M ${x1} ${y1} C ${midX} ${y1}, ${midX} ${y2}, ${x2} ${y2}`,
        labelX: midX,
        labelY: (y1 + y2) / 2 - 6,
      };
    })
    .filter((edge): edge is CausalFitGraphEdge => edge !== null);

  return { width, height, nodes, edges };
}

function patternFilterRequest(pattern: StatePattern): PatternFilterRequest {
  return {
    family: pattern.family as PatternTab,
    leading_object_type: pattern.leading_object_type,
    state: pattern.state ?? undefined,
    from_state: pattern.from_state ?? undefined,
    to_state: pattern.to_state ?? undefined,
    sequence: [...pattern.sequence],
    eo_edges: pattern.eo_edges.map(({ source, target }) => ({ source, target })),
    oo_edges: pattern.oo_edges.map(({ source, target }) => ({ source, target })),
  };
}

function patternFilterLabel(pattern: PatternFilterRequest): string {
  if (pattern.family === 'inter') {
    return `${pattern.from_state ?? '?'} -> ${pattern.to_state ?? '?'} on ${pattern.leading_object_type}`;
  }
  return `${pattern.state ?? '?'} on ${pattern.leading_object_type}`;
}

function graphMenuPosition(clientX: number, clientY: number): { x: number; y: number } {
  const width = typeof window === 'undefined' ? 1280 : window.innerWidth;
  const height = typeof window === 'undefined' ? 800 : window.innerHeight;
  return {
    x: Math.min(Math.max(clientX, 12), Math.max(12, width - 280)),
    y: Math.min(Math.max(clientY, 12), Math.max(12, height - 180)),
  };
}

function textAttributeKey(attribute: Pick<TextAttributeOption, 'scope' | 'name'>): string {
  return `${attribute.scope}::${attribute.name}`;
}

function clampInteger(value: string, min: number, max: number): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return min;
  }
  return Math.min(max, Math.max(min, parsed));
}

function safeFilePart(value: string): string {
  return (
    value
      .trim()
      .replace(/[^A-Za-z0-9_-]+/g, '-')
      .replace(/^-|-$/g, '') || 'objects'
  );
}

function withLeadingObjectTypeClause(query: string, leadingObjectType: string): string {
  const clause = `FOR LEADING OBJECT TYPE '${escapeSqlString(leadingObjectType)}'`;
  const stateHeader =
    /^\s*STATE\s+([A-Za-z_][A-Za-z0-9_-]*)(?:\s+FOR\s+LEADING\s+OBJECT\s+TYPE\s+(?:"(?:[^"]|"")*"|'(?:[^']|'')*'|[A-Za-z_][A-Za-z0-9_-]*))?\s+AS\s+CASE/im;

  if (stateHeader.test(query)) {
    return query.replace(stateHeader, (_match, attribute: string) => {
      const leadingWhitespace = query.match(/^\s*/)?.[0] ?? '';
      return `${leadingWhitespace}STATE ${attribute} ${clause} AS CASE`;
    });
  }

  return `STATE state ${clause} AS CASE\n  WHEN event.type IS NOT NULL THEN 'State'\nEND`;
}

function escapeSqlString(value: string): string {
  return value.replace(/'/g, "''");
}

function wrapGraphLabel(label: string, maxLineLength: number, maxLines: number): string[] {
  const chunks = label
    .trim()
    .replace(/\s+\[/g, '\n[')
    .split('\n')
    .map((chunk) => chunk.replace(/\s+/g, ' ').trim())
    .filter(Boolean);
  const lines: string[] = [];

  for (const chunk of chunks) {
    let current = '';
    for (const word of chunk.split(' ')) {
      for (const part of splitLongWord(word, maxLineLength)) {
        const candidate = current ? `${current} ${part}` : part;
        if (candidate.length <= maxLineLength) {
          current = candidate;
        } else {
          lines.push(current);
          current = part;
        }
      }
    }
    if (current) {
      lines.push(current);
    }
  }

  if (lines.length <= maxLines) {
    return lines;
  }

  const trimmed = lines.slice(0, maxLines);
  trimmed[maxLines - 1] = `${trimmed[maxLines - 1].slice(0, maxLineLength - 3)}...`;
  return trimmed;
}

function splitLongWord(word: string, maxLineLength: number): string[] {
  const parts: string[] = [];
  for (let index = 0; index < word.length; index += maxLineLength) {
    parts.push(word.slice(index, index + maxLineLength));
  }
  return parts.length > 0 ? parts : [''];
}

function normalizeTimeRange(
  startMs: number | null,
  endMs: number | null,
  minMs?: number,
  maxMs?: number,
): TimeRangeFilterRequest | null {
  if (startMs === null && endMs === null) {
    return null;
  }

  let start = startMs ?? minMs;
  let end = endMs ?? maxMs;
  if (start === undefined && end === undefined) {
    return null;
  }
  if (start !== undefined && end !== undefined && start > end) {
    [start, end] = [end, start];
  }

  const normalized: TimeRangeFilterRequest = {};
  if (start !== undefined && start !== minMs) {
    normalized.start_ms = start;
  }
  if (end !== undefined && end !== maxMs) {
    normalized.end_ms = end;
  }

  return normalized.start_ms === undefined && normalized.end_ms === undefined ? null : normalized;
}

function toDateTimeLocalInput(timeMs?: number): string {
  if (timeMs === undefined || !Number.isFinite(timeMs)) {
    return '';
  }
  const date = new Date(timeMs);
  const pad = (value: number) => value.toString().padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`;
}

function fromDateTimeLocalInput(value: string): number | null {
  if (!value) {
    return null;
  }
  const timeMs = new Date(value).getTime();
  return Number.isFinite(timeMs) ? timeMs : null;
}

function timeRangeLabel(range: TimeRangeFilterRequest): string {
  const start = range.start_ms !== undefined ? formatDateTime(range.start_ms) : 'start';
  const end = range.end_ms !== undefined ? formatDateTime(range.end_ms) : 'end';
  return `${start} -> ${end}`;
}

function timeFilterCurve(
  buckets: FilterTimeBucket[],
  selectedStartMs: number | null,
  selectedEndMs: number | null,
): TimeFilterCurve | null {
  if (buckets.length === 0) {
    return null;
  }
  const width = 640;
  const height = 180;
  const padding = { left: 10, right: 10, top: 14, bottom: 32 };
  const maxCount = Math.max(1, ...buckets.map((bucket) => bucket.count));
  const points = buckets.map((bucket, index) => ({
    x:
      padding.left +
      (index / Math.max(buckets.length - 1, 1)) * (width - padding.left - padding.right),
    y:
      height - padding.bottom - (bucket.count / maxCount) * (height - padding.top - padding.bottom),
  }));
  const path = smoothPath(points);
  const baseline = height - padding.bottom;
  const rangeStartMs = buckets[0].start_ms;
  const rangeEndMs = buckets[buckets.length - 1].end_ms;
  const span = Math.max(rangeEndMs - rangeStartMs, 1);
  const selectedStart = selectedStartMs ?? rangeStartMs;
  const selectedEnd = selectedEndMs ?? rangeEndMs;
  const selectedStartX =
    padding.left +
    ((Math.min(selectedStart, selectedEnd) - rangeStartMs) / span) *
      (width - padding.left - padding.right);
  const selectedEndX =
    padding.left +
    ((Math.max(selectedStart, selectedEnd) - rangeStartMs) / span) *
      (width - padding.left - padding.right);
  const areaPath =
    path && points.length > 0
      ? `${path} L ${points[points.length - 1].x} ${baseline} L ${points[0].x} ${baseline} Z`
      : '';
  return {
    width,
    height,
    path,
    areaPath,
    startLabel: formatShortDate(buckets[0].start_ms),
    endLabel: formatShortDate(buckets[buckets.length - 1].end_ms),
    selectedStartX: Math.min(Math.max(selectedStartX, padding.left), width - padding.right),
    selectedEndX: Math.min(Math.max(selectedEndX, padding.left), width - padding.right),
    selectionTop: padding.top,
    selectionBottom: baseline,
  };
}

function timeFrequencyChart(
  buckets: TimeFrequencyBucket[],
  states: string[],
): TimeFrequencyChart | null {
  if (buckets.length === 0 || states.length === 0) {
    return null;
  }
  const width = 790;
  const height = 270;
  const plot = { left: 48, right: 740, top: 28, bottom: 220 };
  const colorByState = new Map(
    states.map((state, index) => [state, CHART_COLORS[index % CHART_COLORS.length]]),
  );
  const percentageByBucket = buckets.map(
    (bucket) => new Map(bucket.percentages.map((entry) => [entry.state, entry.percentage])),
  );
  const series = states.map((state) => {
    const points = buckets.map((bucket, index) => {
      const ratio = index / Math.max(buckets.length - 1, 1);
      const percentage = percentageByBucket[index].get(state) ?? 0;
      return {
        x: plot.left + ratio * (plot.right - plot.left),
        y: plot.bottom - (percentage / 100) * (plot.bottom - plot.top),
      };
    });
    const path = smoothPath(points);
    const areaPath =
      path && points.length > 0
        ? `${path} L ${points[points.length - 1].x} ${plot.bottom} L ${points[0].x} ${plot.bottom} Z`
        : '';
    return {
      state,
      color: colorByState.get(state) ?? CHART_COLORS[0],
      path,
      areaPath,
      latest: percentageByBucket[percentageByBucket.length - 1].get(state) ?? 0,
    };
  });

  return {
    width,
    height,
    startLabel: formatShortDate(buckets[0].start_ms),
    endLabel: formatShortDate(buckets[buckets.length - 1].end_ms),
    yTicks: [0, 25, 50, 75, 100],
    series,
  };
}

function performanceSpectrumChart(
  samples: TimePerformanceSample[],
): PerformanceSpectrumChart | null {
  const width = 800;
  const height = 230;
  if (samples.length === 0) {
    return {
      width,
      height,
      points: [],
      minLabel: formatDuration(null),
      maxLabel: formatDuration(null),
      medianX: null,
    };
  }
  const sorted = [...samples].sort((left, right) => left.duration_ms - right.duration_ms);
  const min = sorted[0].duration_ms;
  const max = sorted[sorted.length - 1].duration_ms;
  const span = Math.max(max - min, 1);
  const plot = { left: 54, right: 748, top: 28, bottom: 176 };
  const points = sorted.map((sample, index) => {
    const x = plot.left + ((sample.duration_ms - min) / span) * (plot.right - plot.left);
    const band = index % 7;
    return {
      sample,
      x,
      y: plot.bottom - 18 - band * 18,
      radius: Math.max(3, Math.min(7, 3 + Math.log2(sample.duration_ms / 60_000 + 1))),
    };
  });
  const medianDuration = sorted[Math.floor(sorted.length / 2)].duration_ms;
  const medianX = plot.left + ((medianDuration - min) / span) * (plot.right - plot.left);

  return {
    width,
    height,
    points,
    minLabel: formatDuration(min),
    maxLabel: formatDuration(max),
    medianX,
  };
}

function smoothPath(points: { x: number; y: number }[]): string {
  if (points.length === 0) {
    return '';
  }
  if (points.length === 1) {
    return `M ${round(points[0].x)} ${round(points[0].y)}`;
  }

  const commands = [`M ${round(points[0].x)} ${round(points[0].y)}`];
  for (let index = 0; index < points.length - 1; index += 1) {
    const previous = points[Math.max(0, index - 1)];
    const current = points[index];
    const next = points[index + 1];
    const following = points[Math.min(points.length - 1, index + 2)];
    const cp1x = current.x + (next.x - previous.x) / 6;
    const cp1y = current.y + (next.y - previous.y) / 6;
    const cp2x = next.x - (following.x - current.x) / 6;
    const cp2y = next.y - (following.y - current.y) / 6;
    commands.push(
      `C ${round(cp1x)} ${round(cp1y)}, ${round(cp2x)} ${round(cp2y)}, ${round(next.x)} ${round(
        next.y,
      )}`,
    );
  }
  return commands.join(' ');
}

function validStateSelection(current: string, states: string[], fallback: string): string {
  if (current && states.includes(current)) {
    return current;
  }
  if (fallback && states.includes(fallback)) {
    return fallback;
  }
  return states[0] ?? '';
}

function formatDateTime(timeMs: number): string {
  if (!Number.isFinite(timeMs)) {
    return '-';
  }
  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: 'short',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(timeMs));
}

function formatShortDate(timeMs: number): string {
  if (!Number.isFinite(timeMs)) {
    return '-';
  }
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: '2-digit',
    year: 'numeric',
  }).format(new Date(timeMs));
}

function formatDuration(durationMs?: number | null): string {
  if (durationMs === undefined || durationMs === null || !Number.isFinite(durationMs)) {
    return '-';
  }
  const absolute = Math.max(0, durationMs);
  const minutes = absolute / 60_000;
  if (minutes < 1) {
    return `${Math.round(absolute / 1000)}s`;
  }
  if (minutes < 90) {
    return `${round(minutes)}m`;
  }
  const hours = minutes / 60;
  if (hours < 48) {
    return `${round(hours)}h`;
  }
  return `${round(hours / 24)}d`;
}

function round(value: number): number {
  return Math.round(value * 100) / 100;
}

const CHART_COLORS = [
  '#1d4f49',
  '#4678a0',
  '#b45f1a',
  '#7b4fa3',
  '#2f7d3d',
  '#a33f5f',
  '#5661a8',
  '#8b6b23',
];
