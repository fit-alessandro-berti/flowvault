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

export const appCorrelationMethods = {
  reloadStateCorrelation(this: any): void {
    this.loadStateCorrelation();
  },

  correlationRows(this: any, analysis: StateCorrelationResult): StateCorrelationRow[] {
    return analysis.rows;
  },

  formatCorrelation(this: any, value: number): string {
    const rounded = Math.round(value * 1000) / 1000;
    const formatted = rounded.toFixed(3);
    return rounded > 0 ? `+${formatted}` : formatted;
  },

  formatFeatureNumber(this: any, value: number): string {
    if (!Number.isFinite(value)) {
      return '0';
    }
    if (Math.abs(value - Math.round(value)) < 0.000_000_1) {
      return Math.round(value).toLocaleString();
    }
    return value.toLocaleString(undefined, {
      maximumFractionDigits: 3,
    });
  },

  strengthLabel(this: any, value: number): string {
    return `${Math.round(value * 100)}%`;
  },

  correlationCellStyle(this: any, row: StateCorrelationRow): string {
    return correlationHeatStyle(row.correlation);
  },

  stateDistributionLabel(this: any, analysis: StateCorrelationResult): string {
    return analysis.state_distribution
      .map((entry) => `${entry.state}: ${entry.count.toLocaleString()}`)
      .join(', ');
  },

  loadStateCorrelation(this: any): void {
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
  },
};

export type AppCorrelationMethods = typeof appCorrelationMethods;
