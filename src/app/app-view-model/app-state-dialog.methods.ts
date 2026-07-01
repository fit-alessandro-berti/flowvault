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

export const appStateDialogMethods = {
  openStateDialog(this: any): void {
    if (!this.documentHandle) {
      return;
    }

    const presets = this.stateQueryPresets();
    const selectedPreset =
      presets.find((preset: StateQueryPreset) => preset.id === this.selectedPresetId()) ?? presets[0];

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
  },

  closeStateDialog(this: any): void {
    this.isStateDialogOpen.set(false);
  },

  selectStatePreset(this: any, preset: StateQueryPreset): void {
    const leadingObjectType = this.validLeadingObjectType(preset.leadingObjectType);
    this.selectedPresetId.set(preset.id);
    this.selectedLeadingObjectType.set(leadingObjectType);
    this.stateQueryDraft.set(withLeadingObjectTypeClause(preset.query, leadingObjectType));
  },

  selectPersistedStateExpression(this: any): void {
    const expression = this.persistedStateExpression();
    if (!expression) {
      return;
    }

    this.ensureLeadingObjectTypeSelection();
    this.selectedPresetId.set(SAVED_STATE_PRESET_ID);
    this.stateQueryDraft.set(
      withLeadingObjectTypeClause(expression, this.selectedLeadingObjectType()),
    );
  },

  selectLlmStateExpression(this: any): void {
    this.ensureLeadingObjectTypeSelection();
    this.selectedPresetId.set(LLM_STATE_PRESET_ID);
    if (!this.stateQueryDraft().trim()) {
      this.stateQueryDraft.set(defaultStateQuery(this.selectedLeadingObjectType()));
    }
  },

  onStateQueryDraftChange(this: any, event: Event): void {
    this.stateQueryDraft.set((event.target as HTMLTextAreaElement).value);
  },

  onLlmStatePromptChange(this: any, event: Event): void {
    this.llmStatePrompt.set((event.target as HTMLTextAreaElement).value);
  },

  onLeadingObjectTypeChange(this: any, event: Event): void {
    const leadingObjectType = (event.target as HTMLSelectElement).value;
    this.selectedLeadingObjectType.set(leadingObjectType);
    this.stateQueryDraft.set(
      withLeadingObjectTypeClause(this.stateQueryDraft(), leadingObjectType),
    );
  },

  applyStateQuery(this: any): void {
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
      this.stateTransitionKpis.set(null);
      this.lifecycleDetail.set(null);
      this.loadStatePatterns();
      this.activeFeature.set('patterns');
      this.stateMessage.set(
        `Added ${result.attribute} for ${result.leading_object_type} to ${result.assigned_events.toLocaleString()} of ${result.total_events.toLocaleString()} events.`,
      );
      this.isStateDialogOpen.set(false);
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  },

  initializeStatePresetForFile(this: any, fileName: string): void {
    const preset = presetsForFile(fileName)[0];
    if (preset) {
      this.selectStatePreset(preset);
      return;
    }

    this.selectedPresetId.set('');
    this.ensureLeadingObjectTypeSelection();
    this.stateQueryDraft.set(defaultStateQuery(this.selectedLeadingObjectType()));
  },

  ensureLeadingObjectTypeSelection(this: any): void {
    this.selectedLeadingObjectType.set(
      this.validLeadingObjectType(this.selectedLeadingObjectType()),
    );
  },

  validLeadingObjectType(this: any, candidate: string): string {
    const options = this.leadingObjectTypeOptions();
    if (candidate && options.includes(candidate)) {
      return candidate;
    }
    return options[0] ?? this.filterOptions().object_types[0] ?? candidate;
  },
};

export type AppStateDialogMethods = typeof appStateDialogMethods;
