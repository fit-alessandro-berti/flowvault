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

export const appTimePerspectiveMethods = {
  reloadTimePerspective(this: any): void {
    this.loadTimePerspective();
  },

  onTimePerspectiveObjectTypeChange(this: any, event: Event): void {
    this.timePerspectiveObjectType.set((event.target as HTMLSelectElement).value);
    this.timePerspectiveFromState.set('');
    this.timePerspectiveToState.set('');
    this.loadTimePerspective();
  },

  onTimePerspectiveFromStateChange(this: any, event: Event): void {
    const fromState = (event.target as HTMLSelectElement).value;
    this.timePerspectiveFromState.set(fromState);
    if (this.timePerspectiveToState() === fromState) {
      this.timePerspectiveToState.set(
        this.timePerspective()?.states.find((state: string) => state !== fromState) ?? '',
      );
    }
    this.loadTimePerspective();
  },

  onTimePerspectiveToStateChange(this: any, event: Event): void {
    this.timePerspectiveToState.set((event.target as HTMLSelectElement).value);
    this.loadTimePerspective();
  },

  onTimePerspectiveRoundtripChange(this: any, event: Event): void {
    this.timePerspectiveRoundtrip.set((event.target as HTMLInputElement).checked);
    this.loadTimePerspective();
  },

  performanceModeLabel(this: any, analysis: TimePerspectiveResult): string {
    return analysis.performance.roundtrip
      ? `${analysis.performance.from_state} -> ${analysis.performance.to_state} -> ${analysis.performance.from_state}`
      : `${analysis.performance.from_state} -> ${analysis.performance.to_state}`;
  },

  durationLabel(this: any, durationMs?: number | null): string {
    return formatDuration(durationMs);
  },

  timeLabel(this: any, timeMs: number): string {
    return formatDateTime(timeMs);
  },

  loadTimePerspective(this: any): void {
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
          this.timePerspectiveFromState(),
        ),
      );
      this.errorMessage.set('');
    } catch (error) {
      this.timePerspective.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  },

  ensureTimePerspectiveObjectType(this: any): void {
    const selected = this.selectedObjectTypes();
    const current = this.timePerspectiveObjectType();
    if (current && selected.includes(current)) {
      return;
    }
    this.timePerspectiveObjectType.set(selected[0] ?? this.filterOptions().object_types[0] ?? '');
  },

  timePerspectiveRequestJson(this: any): string {
    return JSON.stringify({
      object_type: this.timePerspectiveObjectType() || undefined,
      from_state: this.timePerspectiveFromState() || undefined,
      to_state: this.timePerspectiveToState() || undefined,
      roundtrip: this.timePerspectiveRoundtrip(),
      buckets: 32,
    });
  },
};

export type AppTimePerspectiveMethods = typeof appTimePerspectiveMethods;
