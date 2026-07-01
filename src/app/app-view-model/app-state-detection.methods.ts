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

export const appStateDetectionMethods = {
  onStateDetectionObjectTypeChange(this: any, event: Event): void {
    this.stateDetectionObjectType.set((event.target as HTMLSelectElement).value);
    this.stateDetectionAnalysis.set(null);
  },

  onStateDetectionWindowSizeChange(this: any, event: Event): void {
    this.stateDetectionWindowSize.set(
      clampInteger((event.target as HTMLInputElement).value, 1, 30),
    );
    this.stateDetectionAnalysis.set(null);
  },

  onStateDetectionSomWidthChange(this: any, event: Event): void {
    this.stateDetectionSomWidth.set(clampInteger((event.target as HTMLInputElement).value, 2, 12));
    this.stateDetectionAnalysis.set(null);
  },

  onStateDetectionSomHeightChange(this: any, event: Event): void {
    this.stateDetectionSomHeight.set(clampInteger((event.target as HTMLInputElement).value, 2, 12));
    this.stateDetectionAnalysis.set(null);
  },

  onStateDetectionColorAttributeChange(this: any, event: Event): void {
    this.stateDetectionColorAttribute.set((event.target as HTMLSelectElement).value);
    this.stateDetectionAnalysis.set(null);
  },

  runStateDetection(this: any): void {
    this.loadStateDetection();
  },

  applyStateDetection(this: any): void {
    if (!this.documentHandle || !this.stateDetectionAnalysis()) {
      return;
    }

    try {
      this.ensureStateDetectionObjectType();
      const result = JSON.parse(
        this.documentHandle.applyStateDetection(this.stateDetectionRequestJson()),
      ) as StateQueryResult;
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
      this.loadStateAwareOcdfg();
      this.stateDetectionCellDetail.set(null);
      this.activeFeature.set('patterns');
      this.stateMessage.set(
        `Added ${result.attribute} for ${result.leading_object_type} from ${this.stateDetectionSomWidth()} x ${this.stateDetectionSomHeight()} SOM windows to ${result.assigned_events.toLocaleString()} of ${result.total_events.toLocaleString()} events.`,
      );
      this.errorMessage.set('');
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  },

  downloadStateFeatureTable(this: any): void {
    if (!this.documentHandle) {
      return;
    }

    try {
      this.ensureStateDetectionObjectType();
      const csv = this.documentHandle.stateFeatureTableCsv(this.stateDetectionRequestJson());
      this.downloadNamed(
        csv,
        'text/csv',
        `${exportBaseName(this.fileName())}-${safeFilePart(this.stateDetectionObjectType())}-features.csv`,
      );
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  },

  previewColumns(this: any, analysis: StateDetectionResult, limit = 10): string[] {
    return analysis.feature_columns.slice(0, limit);
  },

  previewRows(this: any, analysis: StateDetectionResult): StateDetectionPreviewRow[] {
    return analysis.table_preview.slice(0, 15);
  },

  previewValues(this: any, row: StateDetectionPreviewRow, limit = 10): number[] {
    return row.values.slice(0, limit);
  },

  hiddenFeatureColumnCount(this: any, analysis: StateDetectionResult, limit = 10): number {
    return Math.max(analysis.feature_columns.length - limit, 0);
  },

  somGridColumns(this: any, analysis: StateDetectionResult): string {
    return `repeat(${analysis.som_width}, minmax(0, 1fr))`;
  },

  somCellStyle(this: any, cell: StateDetectionSomCell): string {
    const lightness = Math.round(94 - cell.color_value * 44);
    return `hsl(186 58% ${lightness}%)`;
  },

  somCellTitle(this: any, cell: StateDetectionSomCell): string {
    const activity = cell.dominant_activity ? ` | ${cell.dominant_activity}` : '';
    return `${cell.label}: ${cell.count.toLocaleString()} windows | ${cell.color_label}${activity}`;
  },

  topSomTransitions(
    this: any,
    transitions: StateDetectionSomTransition[],
    limit = 12,
  ): StateDetectionSomTransition[] {
    return transitions.slice(0, limit);
  },

  transitionLabel(this: any, transition: StateDetectionSomTransition): string {
    return `S${transition.source_x + 1}-${transition.source_y + 1} -> S${transition.target_x + 1}-${transition.target_y + 1}`;
  },

  percent(this: any, value: number): string {
    return `${Math.round(value * 1000) / 10}%`;
  },

  openStateDetectionCell(this: any, cell: StateDetectionSomCell): void {
    if (!this.documentHandle || !this.stateDetectionAnalysis()) {
      return;
    }

    try {
      const request = {
        ...JSON.parse(this.stateDetectionRequestJson()),
        cell_x: cell.x,
        cell_y: cell.y,
      };
      const detail = JSON.parse(
        this.documentHandle.stateDetectionCellJson(JSON.stringify(request)),
      ) as StateDetectionCellDetail;
      this.stateDetectionCellDetail.set(detail);
      this.stateDetectionCellTab.set('dfg');
      this.errorMessage.set('');
    } catch (error) {
      this.stateDetectionCellDetail.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  },

  closeStateDetectionCell(this: any): void {
    this.stateDetectionCellDetail.set(null);
    this.stateDetectionCellTab.set('dfg');
  },

  setStateDetectionCellTab(this: any, tab: StateDetectionCellTab): void {
    this.stateDetectionCellTab.set(tab);
  },

  stateDetectionCellGraphSettings(this: any): ProcessGraphSettings {
    const objectType =
      this.stateDetectionAnalysis()?.object_type ?? this.stateDetectionObjectType();
    return {
      object_types: objectType ? [objectType] : [],
      min_activity_frequency: 1,
      min_path_frequency: 1,
    };
  },

  stateDetectionCellObjectTypes(this: any): string[] {
    const objectType =
      this.stateDetectionAnalysis()?.object_type ?? this.stateDetectionObjectType();
    return objectType ? [objectType] : [];
  },

  ignoreStateDetectionCellGraphSettings(this: any, _settings: ProcessGraphSettings): void {
    return;
  },

  loadStateDetection(this: any): void {
    if (!this.documentHandle) {
      this.stateDetectionAnalysis.set(null);
      return;
    }

    try {
      this.ensureStateDetectionObjectType();
      if (!this.stateDetectionObjectType()) {
        this.stateDetectionAnalysis.set(null);
        return;
      }
      const analysis = JSON.parse(
        this.documentHandle.stateDetectionJson(this.stateDetectionRequestJson()),
      ) as StateDetectionResult;
      this.stateDetectionAnalysis.set(analysis);
      this.stateDetectionColorOptions.set(analysis.color_attributes);
      this.stateDetectionColorAttribute.set(analysis.color_attribute);
      this.stateDetectionCellDetail.set(null);
      this.errorMessage.set('');
    } catch (error) {
      this.stateDetectionAnalysis.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  },

  stateDetectionRequestJson(this: any): string {
    return JSON.stringify({
      object_type: this.stateDetectionObjectType(),
      window_size: this.stateDetectionWindowSize(),
      som_width: this.stateDetectionSomWidth(),
      som_height: this.stateDetectionSomHeight(),
      color_attribute: this.stateDetectionColorAttribute(),
    });
  },

  ensureStateDetectionObjectType(this: any): void {
    const selected = this.selectedObjectTypes();
    const current = this.stateDetectionObjectType();
    if (current && selected.includes(current)) {
      return;
    }
    this.stateDetectionObjectType.set(selected[0] ?? this.filterOptions().object_types[0] ?? '');
  },
};

export type AppStateDetectionMethods = typeof appStateDetectionMethods;
