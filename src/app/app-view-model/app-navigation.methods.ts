import { exportBaseName, formatHintForFile } from '../ocel-file';
import { presetsForFile, StateQueryPreset } from '../state-query-presets';
import { providerById, requestChatCompletion, type LlmConfig } from '../llm';
import type { CausalFeatureTableResult, CausalFitResult, LifecycleEventDetail, ObjectLifecycleDetail, ObjectSearchResult, OcelFilterOptions, OcelSummary, ProcessGraph, ProcessGraphSettings, StateCorrelationResult, StateCorrelationRow, StateDetectionPreviewRow, StateDetectionResult, StateDetectionCellDetail, StateDetectionSomCell, StateDetectionSomTransition, StatePattern, StatePatternAnalysis, StatePatternEdge, StateQueryResult, StateTransitionKpiResult, StateTransitionKpiRow, StateDwellKpiRow, StuckStateRow, TextAttributeOption, TimePerspectiveResult } from '../ocel-wasm.service';
import type { DfEdgeFilterRequest, FilterDialogKind, FilterRequest, TextAttributeFilterRequest } from '../models/filter.models';
import type { CausalModelNode, CausalOperation } from '../models/causal.models';
import type { PatternTab, PatternGraph, PatternGraphEdge, PatternGraphNode, PatternVisualization, StaticSampleLog } from '../models/pattern.models';
import type { FeaturePage, StateDetectionCellTab } from '../models/feature.models';
import type { SummaryDisplayValue, SummaryMetric } from '../models/summary.models';
import type { TimeFilterCurve, TransitionMatrixCell } from '../models/time.models';
import { DEFAULT_STATE_DETECTION_COLOR_OPTIONS } from '../helpers/static-data';
import { DEFAULT_LLM_STATE_PROMPT, LLM_CONFIG_STORAGE_KEY, LLM_STATE_PRESET_ID, SAVED_STATE_PRESET_ID, STATE_EXPRESSION_EXAMPLES, STATE_EXPRESSION_STORAGE_KEY, defaultStateQuery, extractStateExpression, readStoredString, writeStoredJson, writeStoredString } from '../helpers/state-expression.helpers';
import { clampInteger, correlationHeatStyle, edgeKey, emptySummaryValue, errorToMessage, graphMenuPosition, graphRequestJson, safeFilePart, sameEdge, selectedPattern, textAttributeKey, toggleSelection, uniqueEdges, withLeadingObjectTypeClause } from '../helpers/common.helpers';
import { canAddCausalEdge, causalFeatureLabel, nextCausalNodeId, parseCausalModelSuggestion, pruneCausalEdges } from '../helpers/causal-model.helpers';
import { patternFilterRequest, wrapGraphLabel } from '../helpers/pattern.helpers';
import { fromDateTimeLocalInput, normalizeTimeRange, timeRangeLabel, toDateTimeLocalInput } from '../helpers/time-range.helpers';
import { formatDateTime, formatDuration, validStateSelection } from '../helpers/time-format.helpers';

export const appNavigationMethods = {
  setActiveFeature(this: any, feature: FeaturePage): void {
    if (
      (feature === 'patterns' ||
        feature === 'correlation' ||
        feature === 'transitionKpis' ||
        feature === 'lifecycle' ||
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
    if (feature === 'transitionKpis' && !this.stateTransitionKpis()) {
      this.loadStateTransitionKpis();
    }
    if (feature === 'lifecycle' && this.lifecycleSearchResults().length === 0) {
      this.loadLifecycleSearch();
    }
    if (feature === 'timePerspective' && !this.timePerspective()) {
      this.loadTimePerspective();
    }
  },

  toggleFilterMenu(this: any): void {
    if (!this.hasDocument()) {
      return;
    }

    this.isFilterMenuOpen.update((isOpen: boolean) => !isOpen);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
  },

  toggleExportMenu(this: any): void {
    if (!this.hasDocument()) {
      return;
    }

    this.isExportMenuOpen.update((isOpen: boolean) => !isOpen);
    this.isFilterMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
  },

  toggleFilterChain(this: any): void {
    if (this.appliedFilters().length === 0) {
      return;
    }

    this.isFilterChainOpen.update((isOpen: boolean) => !isOpen);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.graphFilterMenu.set(null);
  },
};

export type AppNavigationMethods = typeof appNavigationMethods;
