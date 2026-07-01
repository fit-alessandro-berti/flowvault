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

export const appFilterOpenMethods = {
  openActivityFilterDialog(this: any): void {
    this.draftEventTypes.set([...this.selectedEventTypes()]);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.filterDialog.set('activities');
  },

  openObjectTypeFilterDialog(this: any): void {
    this.draftObjectTypes.set([...this.selectedObjectTypes()]);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.filterDialog.set('objectTypes');
  },

  openDfNodeFilterDialog(this: any): void {
    this.draftDfNodes.set([...this.selectedDfNodes()]);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.filterDialog.set('dfNodes');
  },

  openDfEdgeFilterDialog(this: any): void {
    this.draftDfEdges.set([...this.selectedDfEdges()]);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.filterDialog.set('dfEdges');
  },

  openTimeframeFilterDialog(this: any): void {
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
  },

  openTextAttributeFilterDialog(this: any): void {
    const selected = this.selectedTextAttribute();
    const first = selected ?? this.defaultTextAttributeFilter();
    this.draftTextAttributeKey.set(first ? textAttributeKey(first) : '');
    this.draftTextAttributeValues.set(first?.values ?? []);
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.filterDialog.set('textAttributes');
  },

  closeFilterDialog(this: any): void {
    this.filterDialog.set(null);
  },
};

export type AppFilterOpenMethods = typeof appFilterOpenMethods;
