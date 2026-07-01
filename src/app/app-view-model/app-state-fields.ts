import { inject, signal } from '@angular/core';
import { LLM_PROVIDERS, type LlmProviderId } from '../llm';
import { OcelWasmService } from '../ocel-wasm.service';
import type { CausalFeatureTableResult, CausalFitResult, ObjectSearchHit, ObjectLifecycleDetail, OcelDocumentHandle, OcelFilterOptions, OcelSummary, ProcessGraph, ProcessGraphSettings, StateCorrelationResult, StateDetectionCellDetail, StateDetectionColorOption, StateDetectionResult, StatePattern, StatePatternAnalysis, StateTransitionKpiResult, TimePerspectiveResult } from '../ocel-wasm.service';
import type { CausalModelEdge, CausalModelNode } from '../models/causal.models';
import type { DfEdgeFilterRequest, FilterDialogKind, GraphFilterMenu, PatternFilterRequest, TextAttributeFilterRequest, TimeRangeFilterRequest } from '../models/filter.models';
import type { FeaturePage, StateDetectionCellTab } from '../models/feature.models';
import type { PatternTab, PatternVisualization } from '../models/pattern.models';
import { CAUSAL_OPERATIONS, DEFAULT_STATE_DETECTION_COLOR_OPTIONS, STATIC_SAMPLE_LOGS } from '../helpers/static-data';
import { DEFAULT_LLM_STATE_PROMPT, STATE_EXPRESSION_STORAGE_KEY, loadLlmConfig, readStoredString } from '../helpers/state-expression.helpers';
import { emptyGraphSettings } from '../helpers/common.helpers';

export class AppStateFields {
  protected readonly ocelWasm = inject(OcelWasmService);
  protected readonly vm = this;
  protected documentHandle?: OcelDocumentHandle;

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
  protected timeSelectionAnchorMs: number | null = null;
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
  protected readonly stateTransitionKpis = signal<StateTransitionKpiResult | null>(null);
  protected readonly stateTransitionObjectType = signal('');
  protected readonly lifecycleObjectType = signal('');
  protected readonly lifecycleSearchQuery = signal('');
  protected readonly lifecycleSearchResults = signal<ObjectSearchHit[]>([]);
  protected readonly selectedLifecycleObjectId = signal('');
  protected readonly lifecycleDetail = signal<ObjectLifecycleDetail | null>(null);
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
}
