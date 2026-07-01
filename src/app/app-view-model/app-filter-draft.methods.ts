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

export const appFilterDraftMethods = {
  toggleDraftEventType(this: any, eventType: string, event: Event): void {
    this.draftEventTypes.set(
      toggleSelection(
        this.draftEventTypes(),
        eventType,
        (event.target as HTMLInputElement).checked,
      ),
    );
  },

  toggleDraftObjectType(this: any, objectType: string, event: Event): void {
    this.draftObjectTypes.set(
      toggleSelection(
        this.draftObjectTypes(),
        objectType,
        (event.target as HTMLInputElement).checked,
      ),
    );
  },

  toggleDraftDfNode(this: any, activity: string, event: Event): void {
    this.draftDfNodes.set(
      toggleSelection(this.draftDfNodes(), activity, (event.target as HTMLInputElement).checked),
    );
  },

  toggleDraftDfEdge(this: any, edge: DfEdgeFilterRequest, event: Event): void {
    const checked = (event.target as HTMLInputElement).checked;
    const current = this.draftDfEdges();
    const normalizedEdge = { source: edge.source, target: edge.target };
    this.draftDfEdges.set(
      checked
        ? [...current, normalizedEdge].filter(uniqueEdges)
        : current.filter((candidate: DfEdgeFilterRequest) => !sameEdge(candidate, normalizedEdge)),
    );
  },

  onDraftTextAttributeChange(this: any, event: Event): void {
    const key = (event.target as HTMLSelectElement).value;
    this.draftTextAttributeKey.set(key);
    this.draftTextAttributeValues.set([]);
  },

  toggleDraftTextAttributeValue(this: any, value: string, event: Event): void {
    this.draftTextAttributeValues.set(
      toggleSelection(
        this.draftTextAttributeValues(),
        value,
        (event.target as HTMLInputElement).checked,
      ),
    );
  },

  selectAllDraftEventTypes(this: any): void {
    this.draftEventTypes.set([...this.filterOptions().event_types]);
  },

  clearDraftEventTypes(this: any): void {
    this.draftEventTypes.set([]);
  },

  selectAllDraftObjectTypes(this: any): void {
    this.draftObjectTypes.set([...this.filterOptions().object_types]);
  },

  clearDraftObjectTypes(this: any): void {
    this.draftObjectTypes.set([]);
  },

  selectAllDraftDfNodes(this: any): void {
    this.draftDfNodes.set([...this.filterOptions().event_types]);
  },

  clearDraftDfNodes(this: any): void {
    this.draftDfNodes.set([]);
  },

  selectAllDraftDfEdges(this: any): void {
    this.draftDfEdges.set(this.dfEdgeOptions().map(({ source, target }: DfEdgeFilterRequest) => ({ source, target })));
  },

  clearDraftDfEdges(this: any): void {
    this.draftDfEdges.set([]);
  },

  selectAllDraftTextAttributeValues(this: any): void {
    this.draftTextAttributeValues.set([...(this.draftTextAttributeOption()?.values ?? [])]);
  },

  clearDraftTextAttributeValues(this: any): void {
    this.draftTextAttributeValues.set([]);
  },

  draftTextAttributeOption(this: any): TextAttributeOption | null {
    const key = this.draftTextAttributeKey();
    return (
      this.filterOptions().text_attributes.find((option: TextAttributeOption) => textAttributeKey(option) === key) ??
      null
    );
  },

  defaultTextAttributeFilter(this: any): TextAttributeFilterRequest | null {
    const options = this.filterOptions().text_attributes;
    const option =
      options.find((candidate: TextAttributeOption) => candidate.name === 'state' && candidate.scope === 'event') ??
      options[0];
    if (!option) {
      return null;
    }

    return {
      scope: option.scope,
      name: option.name,
      values: [],
    };
  },

  isDraftDfEdgeSelected(this: any, edge: DfEdgeFilterRequest): boolean {
    return this.draftDfEdges().some((candidate: DfEdgeFilterRequest) => sameEdge(candidate, edge));
  },
};

export type AppFilterDraftMethods = typeof appFilterDraftMethods;
