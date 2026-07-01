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

export const appLifecycleMethods = {
  onLifecycleObjectTypeChange(this: any, event: Event): void {
    this.lifecycleObjectType.set((event.target as HTMLSelectElement).value);
    this.selectedLifecycleObjectId.set('');
    this.lifecycleDetail.set(null);
    this.loadLifecycleSearch();
  },

  onLifecycleSearchQueryChange(this: any, event: Event): void {
    this.lifecycleSearchQuery.set((event.target as HTMLInputElement).value);
    this.loadLifecycleSearch();
  },

  selectLifecycleObject(this: any, objectId: string): void {
    this.selectedLifecycleObjectId.set(objectId);
    this.loadLifecycleDetail(objectId);
  },

  loadSelectedLifecycleObject(this: any): void {
    const objectId = this.selectedLifecycleObjectId().trim() || this.lifecycleSearchQuery().trim();
    if (!objectId) {
      return;
    }
    this.selectedLifecycleObjectId.set(objectId);
    this.loadLifecycleDetail(objectId);
  },

  visibleLifecycleEvents(this: any, detail: ObjectLifecycleDetail, limit = 140): LifecycleEventDetail[] {
    return detail.events.slice(0, limit);
  },

  lifecycleHiddenEventCount(this: any, detail: ObjectLifecycleDetail, limit = 140): number {
    return Math.max(detail.events.length - limit, 0);
  },

  lifecycleAttributeLabel(this: any, event: LifecycleEventDetail, limit = 3): string {
    return event.attributes
      .slice(0, limit)
      .map((attribute) => `${attribute.name}: ${String(attribute.value)}`)
      .join(' | ');
  },

  lifecycleRelatedLabel(this: any, event: LifecycleEventDetail, limit = 3): string {
    const related = event.related_objects.slice(0, limit);
    const suffix = event.related_objects.length > limit ? ` +${event.related_objects.length - limit}` : '';
    return related
      .map((object) => `${object.object_type} ${object.object_id}`)
      .join(', ') + suffix;
  },

  loadLifecycleSearch(this: any): void {
    if (!this.documentHandle) {
      this.lifecycleSearchResults.set([]);
      return;
    }

    try {
      this.ensureLifecycleObjectType();
      const result = JSON.parse(
        this.documentHandle.objectSearchJson(this.lifecycleSearchRequestJson()),
      ) as ObjectSearchResult;
      this.lifecycleSearchResults.set(result.objects);
      if (
        result.objects.length > 0 &&
        !result.objects.some((object: { object_id: string }) => object.object_id === this.selectedLifecycleObjectId())
      ) {
        this.selectLifecycleObject(result.objects[0].object_id);
      } else if (result.objects.length === 0) {
        this.selectedLifecycleObjectId.set('');
        this.lifecycleDetail.set(null);
      }
      this.errorMessage.set('');
    } catch (error) {
      this.lifecycleSearchResults.set([]);
      this.errorMessage.set(errorToMessage(error));
    }
  },

  loadLifecycleDetail(this: any, objectId: string): void {
    if (!this.documentHandle) {
      this.lifecycleDetail.set(null);
      return;
    }

    try {
      this.lifecycleDetail.set(
        JSON.parse(this.documentHandle.objectLifecycleDetailJson(objectId)) as ObjectLifecycleDetail,
      );
      this.errorMessage.set('');
    } catch (error) {
      this.lifecycleDetail.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  },

  ensureLifecycleObjectType(this: any): void {
    const selected = this.selectedObjectTypes();
    const current = this.lifecycleObjectType();
    if (current && selected.includes(current)) {
      return;
    }
    this.lifecycleObjectType.set(selected[0] ?? this.filterOptions().object_types[0] ?? '');
  },

  lifecycleSearchRequestJson(this: any): string {
    return JSON.stringify({
      object_type: this.lifecycleObjectType() || undefined,
      query: this.lifecycleSearchQuery() || undefined,
      limit: 40,
    });
  },
};

export type AppLifecycleMethods = typeof appLifecycleMethods;
