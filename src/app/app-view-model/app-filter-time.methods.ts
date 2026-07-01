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

export const appFilterTimeMethods = {
  onDraftTimeStartChange(this: any, event: Event): void {
    this.draftTimeStart.set((event.target as HTMLInputElement).value);
  },

  onDraftTimeEndChange(this: any, event: Event): void {
    this.draftTimeEnd.set((event.target as HTMLInputElement).value);
  },

  resetDraftTimeframe(this: any): void {
    this.draftTimeStart.set(toDateTimeLocalInput(this.filterOptions().time_min_ms));
    this.draftTimeEnd.set(toDateTimeLocalInput(this.filterOptions().time_max_ms));
  },

  startTimeRangeSelection(this: any, event: PointerEvent, curve: TimeFilterCurve): void {
    const timeMs = this.timeMsFromChartEvent(event, curve);
    if (timeMs === null) {
      return;
    }

    event.preventDefault();
    (event.currentTarget as SVGSVGElement).setPointerCapture(event.pointerId);
    this.timeSelectionAnchorMs = timeMs;
    this.isSelectingTimeRange.set(true);
    this.updateDraftTimeRangeFromSelection(timeMs, timeMs);
  },

  moveTimeRangeSelection(this: any, event: PointerEvent, curve: TimeFilterCurve): void {
    if (!this.isSelectingTimeRange() || this.timeSelectionAnchorMs === null) {
      return;
    }
    const timeMs = this.timeMsFromChartEvent(event, curve);
    if (timeMs === null) {
      return;
    }
    event.preventDefault();
    this.updateDraftTimeRangeFromSelection(this.timeSelectionAnchorMs, timeMs);
  },

  endTimeRangeSelection(this: any, event: PointerEvent, curve: TimeFilterCurve): void {
    if (!this.isSelectingTimeRange()) {
      return;
    }
    this.moveTimeRangeSelection(event, curve);
    if ((event.currentTarget as SVGSVGElement).hasPointerCapture(event.pointerId)) {
      (event.currentTarget as SVGSVGElement).releasePointerCapture(event.pointerId);
    }
    this.timeSelectionAnchorMs = null;
    this.isSelectingTimeRange.set(false);
  },

  timeMsFromChartEvent(this: any, event: PointerEvent, curve: TimeFilterCurve): number | null {
    const options = this.filterOptions();
    const minMs = options.time_min_ms;
    const maxMs = options.time_max_ms;
    if (minMs === undefined || maxMs === undefined) {
      return null;
    }
    const rect = (event.currentTarget as SVGSVGElement).getBoundingClientRect();
    const ratio = Math.min(Math.max((event.clientX - rect.left) / rect.width, 0), 1);
    return Math.round(minMs + ratio * (maxMs - minMs));
  },

  updateDraftTimeRangeFromSelection(this: any, startMs: number, endMs: number): void {
    const start = Math.min(startMs, endMs);
    const end = Math.max(startMs, endMs);
    this.draftTimeStart.set(toDateTimeLocalInput(start));
    this.draftTimeEnd.set(toDateTimeLocalInput(end));
  },
};

export type AppFilterTimeMethods = typeof appFilterTimeMethods;
