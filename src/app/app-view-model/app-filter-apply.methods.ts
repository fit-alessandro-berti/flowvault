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

export const appFilterApplyMethods = {
  applyFilterDialog(this: any): void {
    const dialog = this.filterDialog();

    if (dialog === 'activities') {
      this.selectedEventTypes.set([...this.draftEventTypes()]);
    }
    if (dialog === 'objectTypes') {
      this.selectedObjectTypes.set([...this.draftObjectTypes()]);
    }
    if (dialog === 'dfNodes') {
      this.selectedDfNodes.set([...this.draftDfNodes()]);
    }
    if (dialog === 'dfEdges') {
      this.selectedDfEdges.set([...this.draftDfEdges()]);
    }
    if (dialog === 'textAttributes') {
      const option = this.draftTextAttributeOption();
      const values = this.draftTextAttributeValues();
      this.selectedTextAttribute.set(
        option && values.length > 0
          ? {
              scope: option.scope,
              name: option.name,
              values,
            }
          : null,
      );
    }
    if (dialog === 'timeframe') {
      const range = normalizeTimeRange(
        fromDateTimeLocalInput(this.draftTimeStart()),
        fromDateTimeLocalInput(this.draftTimeEnd()),
        this.filterOptions().time_min_ms,
        this.filterOptions().time_max_ms,
      );
      this.selectedTimeRange.set(range);
    }

    this.filterDialog.set(null);
    this.isFilterChainOpen.set(false);
    this.applyActiveFilter();
  },

  removeFilter(this: any, kind: FilterDialogKind): void {
    if (kind === 'activities') {
      this.selectedEventTypes.set([...this.filterOptions().event_types]);
      this.draftEventTypes.set([...this.filterOptions().event_types]);
    } else if (kind === 'objectTypes') {
      this.selectedObjectTypes.set([...this.filterOptions().object_types]);
      this.draftObjectTypes.set([...this.filterOptions().object_types]);
    } else if (kind === 'dfNodes') {
      this.selectedDfNodes.set([]);
      this.draftDfNodes.set([]);
    } else if (kind === 'dfEdges') {
      this.selectedDfEdges.set([]);
      this.draftDfEdges.set([]);
    } else if (kind === 'timeframe') {
      this.selectedTimeRange.set(null);
      this.draftTimeStart.set('');
      this.draftTimeEnd.set('');
    } else if (kind === 'textAttributes') {
      this.selectedTextAttribute.set(null);
      this.draftTextAttributeKey.set('');
      this.draftTextAttributeValues.set([]);
    } else {
      this.selectedPatternFilters.set([]);
    }

    this.filterDialog.set(null);
    this.graphFilterMenu.set(null);
    this.isFilterChainOpen.set(false);
    this.applyActiveFilter();
  },
};

export type AppFilterApplyMethods = typeof appFilterApplyMethods;
