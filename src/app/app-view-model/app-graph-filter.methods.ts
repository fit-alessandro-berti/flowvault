import { exportBaseName, formatHintForFile } from '../ocel-file';
import { presetsForFile, StateQueryPreset } from '../state-query-presets';
import { providerById, requestChatCompletion, type LlmConfig } from '../llm';
import type { CausalFeatureTableResult, CausalFitResult, LifecycleEventDetail, ObjectLifecycleDetail, ObjectSearchResult, OcelFilterOptions, OcelSummary, ProcessGraph, ProcessGraphSettings, StateCorrelationResult, StateCorrelationRow, StateDetectionPreviewRow, StateDetectionResult, StateDetectionCellDetail, StateDetectionSomCell, StateDetectionSomTransition, StatePattern, StatePatternAnalysis, StatePatternEdge, StateQueryResult, StateTransitionKpiResult, StateTransitionKpiRow, StateDwellKpiRow, StuckStateRow, TextAttributeOption, TimePerspectiveResult } from '../ocel-wasm.service';
import type { DfEdgeFilterRequest, FilterDialogKind, FilterRequest, TextAttributeFilterRequest } from '../models/filter.models';
import type { CausalModelNode, CausalOperation } from '../models/causal.models';
import type { PatternTab, PatternGraph, PatternGraphEdge, PatternGraphNode, PatternVisualization, StaticSampleLog } from '../models/pattern.models';
import type { FeaturePage, StateDetectionCellTab } from '../models/feature.models';
import type { ProcessGraphEdgeFilterEvent, ProcessGraphNodeFilterEvent } from '../process-graph.component';
import type { SummaryDisplayValue, SummaryMetric } from '../models/summary.models';
import type { TimeFilterCurve, TransitionMatrixCell } from '../models/time.models';
import { DEFAULT_STATE_DETECTION_COLOR_OPTIONS } from '../helpers/static-data';
import { DEFAULT_LLM_STATE_PROMPT, LLM_CONFIG_STORAGE_KEY, LLM_STATE_PRESET_ID, SAVED_STATE_PRESET_ID, STATE_EXPRESSION_EXAMPLES, STATE_EXPRESSION_STORAGE_KEY, defaultStateQuery, extractStateExpression, readStoredString, writeStoredJson, writeStoredString } from '../helpers/state-expression.helpers';
import { clampInteger, correlationHeatStyle, edgeKey, emptySummaryValue, errorToMessage, graphMenuPosition, graphRequestJson, safeFilePart, sameEdge, selectedPattern, textAttributeKey, toggleSelection, uniqueEdges, withLeadingObjectTypeClause } from '../helpers/common.helpers';
import { canAddCausalEdge, causalFeatureLabel, nextCausalNodeId, parseCausalModelSuggestion, pruneCausalEdges } from '../helpers/causal-model.helpers';
import { patternFilterRequest, wrapGraphLabel } from '../helpers/pattern.helpers';
import { fromDateTimeLocalInput, normalizeTimeRange, timeRangeLabel, toDateTimeLocalInput } from '../helpers/time-range.helpers';
import { formatDateTime, formatDuration, validStateSelection } from '../helpers/time-format.helpers';

export const appGraphFilterMethods = {
  openGraphNodeFilterMenu(this: any, event: ProcessGraphNodeFilterEvent): void {
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set({
      kind: 'node',
      activity: event.activity,
      ...graphMenuPosition(event.clientX, event.clientY),
    });
  },

  openGraphEdgeFilterMenu(this: any, event: ProcessGraphEdgeFilterEvent): void {
    this.isFilterMenuOpen.set(false);
    this.isExportMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set({
      kind: 'edge',
      source: event.source,
      target: event.target,
      ...graphMenuPosition(event.clientX, event.clientY),
    });
  },

  closeGraphFilterMenu(this: any): void {
    this.graphFilterMenu.set(null);
  },

  applyGraphNodeFilter(this: any, activity: string): void {
    this.selectedDfNodes.set([...new Set([...this.selectedDfNodes(), activity])]);
    this.graphFilterMenu.set(null);
    this.applyActiveFilter();
  },

  applyGraphEdgeFilter(this: any, edge: DfEdgeFilterRequest): void {
    this.selectedDfEdges.set([...this.selectedDfEdges(), edge].filter(uniqueEdges));
    this.graphFilterMenu.set(null);
    this.applyActiveFilter();
  },

  applyStateAwareGraphSettings(this: any, settings: ProcessGraphSettings): void {
    this.stateAwareOcdfgSettings.set(this.sanitizeGraphSettings(settings));
    this.loadStateAwareOcdfg();
  },

  applyTraditionalGraphSettings(this: any, settings: ProcessGraphSettings): void {
    this.traditionalOcdfgSettings.set(this.sanitizeGraphSettings(settings));
    this.loadTraditionalOcdfg();
  },

  loadStateAwareOcdfg(this: any): void {
    if (!this.documentHandle) {
      this.stateAwareOcdfg.set(null);
      return;
    }

    try {
      this.stateAwareOcdfg.set(
        JSON.parse(
          this.documentHandle.filteredStateAwareObjectCentricDirectlyFollowsGraphJson(
            graphRequestJson(this.stateAwareOcdfgSettings()),
          ),
        ) as ProcessGraph,
      );
    } catch (error) {
      this.stateAwareOcdfg.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  },

  loadTraditionalOcdfg(this: any): void {
    if (!this.documentHandle) {
      this.traditionalOcdfg.set(null);
      return;
    }

    try {
      this.traditionalOcdfg.set(
        JSON.parse(
          this.documentHandle.filteredObjectCentricDirectlyFollowsGraphJson(
            graphRequestJson(this.traditionalOcdfgSettings()),
          ),
        ) as ProcessGraph,
      );
    } catch (error) {
      this.traditionalOcdfg.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  },

  applyActiveFilter(this: any): void {
    if (!this.documentHandle) {
      return;
    }

    const filter: FilterRequest = {
      event_types: this.selectedEventTypes(),
      object_types: this.selectedObjectTypes(),
    };
    if (this.selectedDfNodes().length > 0) {
      filter.df_nodes = this.selectedDfNodes();
    }
    if (this.selectedDfEdges().length > 0) {
      filter.df_edges = this.selectedDfEdges();
    }
    const timeRange = this.selectedTimeRange();
    if (timeRange) {
      filter.time_range = timeRange;
    }
    const textAttribute = this.selectedTextAttribute();
    if (textAttribute && textAttribute.values.length > 0) {
      filter.text_attributes = [textAttribute];
    }
    if (this.selectedPatternFilters().length > 0) {
      filter.patterns = this.selectedPatternFilters();
    }

    try {
      const nextSummary = JSON.parse(
        this.documentHandle.applyFilter(JSON.stringify(filter)),
      ) as OcelSummary;

      this.summary.set(nextSummary);
      this.originalSummary.set(
        JSON.parse(this.documentHandle.originalSummaryJson()) as OcelSummary,
      );
      this.stateAwareOcdfgSettings.set(this.sanitizeGraphSettings(this.stateAwareOcdfgSettings()));
      this.traditionalOcdfgSettings.set(
        this.sanitizeGraphSettings(this.traditionalOcdfgSettings()),
      );
      this.loadTraditionalOcdfg();
      this.graphFilterMenu.set(null);
      this.updateStateMessageAfterFilter(nextSummary);
      this.ensureStateDetectionObjectType();
      this.stateDetectionCellDetail.set(null);
      if (this.activeFeature() === 'stateDetection' || this.stateDetectionAnalysis()) {
        this.loadStateDetection();
      }
      this.ensureCausalObjectType();
      if (this.activeFeature() === 'causalModel' || this.causalFeatureTable()) {
        this.loadCausalFeatureTable();
      }

      this.stateCorrelation.set(null);
      this.timePerspective.set(null);
      this.stateTransitionKpis.set(null);
      this.lifecycleDetail.set(null);
      if (nextSummary.stateful_events > 0) {
        this.loadStatePatterns(true);
        if (this.activeFeature() === 'correlation') {
          this.loadStateCorrelation();
        }
        if (this.activeFeature() === 'transitionKpis') {
          this.loadStateTransitionKpis();
        }
        if (this.activeFeature() === 'lifecycle') {
          this.loadLifecycleSearch();
        }
        if (this.activeFeature() === 'timePerspective') {
          this.loadTimePerspective();
        }
      } else {
        this.patternAnalysis.set(null);
        this.stateCorrelation.set(null);
        this.timePerspective.set(null);
        this.stateTransitionKpis.set(null);
        this.lifecycleSearchResults.set([]);
        this.selectedLifecycleObjectId.set('');
        this.lifecycleDetail.set(null);
        this.stateAwareOcdfg.set(null);
        this.selectedIntraPatternId.set('');
        this.selectedInterPatternId.set('');
        this.activePatternTab.set('intra');
        this.fullScreenPattern.set(null);
      }
      this.errorMessage.set('');
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  },

  updateStateMessageAfterFilter(this: any, summary: OcelSummary): void {
    const originalSummary = this.originalSummary();

    if (!originalSummary?.stateful_events) {
      this.stateMessage.set('');
      return;
    }

    if (summary.stateful_events > 0) {
      this.stateMessage.set(
        `State retained on ${summary.stateful_events.toLocaleString()} of ${summary.events.toLocaleString()} active events.`,
      );
      return;
    }

    this.stateMessage.set('State is retained in the original log, but no active events match it.');
  },

  resetGraphSettings(this: any, objectTypes: string[]): void {
    const settings = {
      object_types: [...objectTypes],
      min_activity_frequency: 1,
      min_path_frequency: 1,
    };
    this.stateAwareOcdfgSettings.set(settings);
    this.traditionalOcdfgSettings.set(settings);
  },

  sanitizeGraphSettings(this: any, settings: ProcessGraphSettings): ProcessGraphSettings {
    const availableObjectTypes = new Set(this.selectedObjectTypes());
    const objectTypes = settings.object_types.filter((objectType) =>
      availableObjectTypes.has(objectType),
    );

    return {
      object_types: objectTypes,
      min_activity_frequency: Math.max(1, Math.round(settings.min_activity_frequency || 1)),
      min_path_frequency: Math.max(1, Math.round(settings.min_path_frequency || 1)),
    };
  },
};

export type AppGraphFilterMethods = typeof appGraphFilterMethods;
