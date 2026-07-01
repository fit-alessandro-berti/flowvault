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

export const appTransitionMethods = {
  reloadStateTransitionKpis(this: any): void {
    this.loadStateTransitionKpis();
  },

  onStateTransitionObjectTypeChange(this: any, event: Event): void {
    this.stateTransitionObjectType.set((event.target as HTMLSelectElement).value);
    this.loadStateTransitionKpis();
  },

  transitionCount(this: any, analysis: StateTransitionKpiResult): number {
    return analysis.transitions.reduce((total, transition) => total + transition.count, 0);
  },

  topTransitionRows(this: any, analysis: StateTransitionKpiResult, limit = 12): StateTransitionKpiRow[] {
    return analysis.transitions.slice(0, limit);
  },

  topDwellRows(this: any, analysis: StateTransitionKpiResult, limit = 8): StateDwellKpiRow[] {
    return analysis.dwell.slice(0, limit);
  },

  topRecoveryRows(this: any, analysis: StateTransitionKpiResult, limit = 8): StateTransitionKpiRow[] {
    return analysis.recovery.slice(0, limit);
  },

  topStuckRows(this: any, analysis: StateTransitionKpiResult, limit = 12): StuckStateRow[] {
    return analysis.stuck.slice(0, limit);
  },

  matrixCellStyle(this: any, cell: TransitionMatrixCell): string {
    const lightness = Math.round(96 - cell.intensity * 48);
    return `background: hsl(176 46% ${lightness}%);`;
  },

  loadStateTransitionKpis(this: any): void {
    if (!this.documentHandle) {
      this.stateTransitionKpis.set(null);
      return;
    }

    try {
      this.ensureStateTransitionObjectType();
      const analysis = JSON.parse(
        this.documentHandle.stateTransitionKpisJson(this.stateTransitionKpisRequestJson()),
      ) as StateTransitionKpiResult;
      this.stateTransitionKpis.set(analysis);
      this.stateTransitionObjectType.set(analysis.object_type);
      this.errorMessage.set('');
    } catch (error) {
      this.stateTransitionKpis.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  },

  ensureStateTransitionObjectType(this: any): void {
    const selected = this.selectedObjectTypes();
    const current = this.stateTransitionObjectType();
    if (current && selected.includes(current)) {
      return;
    }
    this.stateTransitionObjectType.set(
      selected[0] ?? this.filterOptions().object_types[0] ?? '',
    );
  },

  stateTransitionKpisRequestJson(this: any): string {
    return JSON.stringify({
      object_type: this.stateTransitionObjectType() || undefined,
      stuck_limit: 25,
    });
  },
};

export type AppTransitionMethods = typeof appTransitionMethods;
