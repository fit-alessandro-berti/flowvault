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

export const appFileImportMethods = {
  async onFileSelected(this: any, event: Event): Promise<void> {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';

    if (file) {
      await this.importFile(file);
    }
  },

  onDragOver(this: any, event: DragEvent): void {
    event.preventDefault();
    this.isDragging.set(true);
  },

  onDragLeave(this: any, event: DragEvent): void {
    if (event.currentTarget === event.target) {
      this.isDragging.set(false);
    }
  },

  async onDrop(this: any, event: DragEvent): Promise<void> {
    event.preventDefault();
    this.isDragging.set(false);

    const file = event.dataTransfer?.files?.[0];
    if (file) {
      await this.importFile(file);
    }
  },

  async importSampleLog(this: any, sample: StaticSampleLog): Promise<void> {
    await this.importSource(sample.fileName, async () => {
      const response = await fetch(new URL(sample.path, document.baseURI));
      if (!response.ok) {
        throw new Error(`Could not load bundled sample '${sample.fileName}'.`);
      }
      return response.arrayBuffer();
    });
  },

  async importFile(this: any, file: File): Promise<void> {
    await this.importSource(file.name, () => file.arrayBuffer());
  },

  async importSource(
    this: any,
    fileName: string,
    readInput: () => Promise<ArrayBuffer>,
  ): Promise<void> {
    this.isLoading.set(true);
    this.errorMessage.set('');

    try {
      const input = await readInput();
      const imported = await this.ocelWasm.importDocument(input, formatHintForFile(fileName));

      this.documentHandle?.free();
      this.documentHandle = imported.document;
      this.fileName.set(fileName);
      this.summary.set(imported.summary);
      this.originalSummary.set(imported.originalSummary);
      this.filterOptions.set(imported.filterOptions);
      this.selectedEventTypes.set(imported.filterOptions.event_types);
      this.selectedObjectTypes.set(imported.filterOptions.object_types);
      this.selectedDfNodes.set([]);
      this.selectedDfEdges.set([]);
      this.selectedTimeRange.set(null);
      this.selectedTextAttribute.set(null);
      this.selectedPatternFilters.set([]);
      this.draftEventTypes.set(imported.filterOptions.event_types);
      this.draftObjectTypes.set(imported.filterOptions.object_types);
      this.draftDfNodes.set([]);
      this.draftDfEdges.set([]);
      this.draftTimeStart.set('');
      this.draftTimeEnd.set('');
      this.draftTextAttributeKey.set('');
      this.draftTextAttributeValues.set([]);
      this.filterDialog.set(null);
      this.isFilterMenuOpen.set(false);
      this.isExportMenuOpen.set(false);
      this.isFilterChainOpen.set(false);
      this.graphFilterMenu.set(null);
      this.stateMessage.set('');
      this.patternAnalysis.set(null);
      this.stateCorrelation.set(null);
      this.timePerspective.set(null);
      this.timePerspectiveObjectType.set(imported.filterOptions.object_types[0] ?? '');
      this.timePerspectiveFromState.set('');
      this.timePerspectiveToState.set('');
      this.timePerspectiveRoundtrip.set(false);
      this.stateTransitionKpis.set(null);
      this.stateTransitionObjectType.set(imported.filterOptions.object_types[0] ?? '');
      this.lifecycleObjectType.set(imported.filterOptions.object_types[0] ?? '');
      this.lifecycleSearchQuery.set('');
      this.lifecycleSearchResults.set([]);
      this.selectedLifecycleObjectId.set('');
      this.lifecycleDetail.set(null);
      this.stateAwareOcdfg.set(null);
      this.traditionalOcdfg.set(null);
      this.stateDetectionAnalysis.set(null);
      this.stateDetectionObjectType.set(imported.filterOptions.object_types[0] ?? '');
      this.stateDetectionWindowSize.set(4);
      this.stateDetectionSomWidth.set(3);
      this.stateDetectionSomHeight.set(3);
      this.stateDetectionColorAttribute.set('__window_count');
      this.stateDetectionColorOptions.set(DEFAULT_STATE_DETECTION_COLOR_OPTIONS);
      this.stateDetectionCellDetail.set(null);
      this.stateDetectionCellTab.set('dfg');
      this.causalObjectType.set(imported.filterOptions.object_types[0] ?? '');
      this.causalFeatureTable.set(null);
      this.causalNodes.set([]);
      this.causalEdges.set([]);
      this.causalLatentDraft.set('');
      this.causalFit.set(null);
      this.causalMessage.set('');
      this.isGeneratingCausalModel.set(false);
      this.resetGraphSettings(imported.filterOptions.object_types);
      this.loadTraditionalOcdfg();
      this.selectedIntraPatternId.set('');
      this.selectedInterPatternId.set('');
      this.activePatternTab.set('intra');
      this.activeFeature.set('statistics');
      this.fullScreenPattern.set(null);
      this.isStateDialogOpen.set(false);
      this.initializeStatePresetForFile(fileName);
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
      this.summary.set(null);
      this.originalSummary.set(null);
      this.filterOptions.set({
        event_types: [],
        object_types: [],
        text_attributes: [],
        time_buckets: [],
      });
      this.selectedEventTypes.set([]);
      this.selectedObjectTypes.set([]);
      this.selectedDfNodes.set([]);
      this.selectedDfEdges.set([]);
      this.selectedTimeRange.set(null);
      this.selectedTextAttribute.set(null);
      this.selectedPatternFilters.set([]);
      this.draftEventTypes.set([]);
      this.draftObjectTypes.set([]);
      this.draftDfNodes.set([]);
      this.draftDfEdges.set([]);
      this.draftTimeStart.set('');
      this.draftTimeEnd.set('');
      this.draftTextAttributeKey.set('');
      this.draftTextAttributeValues.set([]);
      this.filterDialog.set(null);
      this.isFilterMenuOpen.set(false);
      this.isExportMenuOpen.set(false);
      this.isFilterChainOpen.set(false);
      this.graphFilterMenu.set(null);
      this.selectedLeadingObjectType.set('');
      this.fileName.set(fileName);
      this.documentHandle?.free();
      this.documentHandle = undefined;
      this.stateMessage.set('');
      this.patternAnalysis.set(null);
      this.stateCorrelation.set(null);
      this.timePerspective.set(null);
      this.timePerspectiveObjectType.set('');
      this.timePerspectiveFromState.set('');
      this.timePerspectiveToState.set('');
      this.timePerspectiveRoundtrip.set(false);
      this.stateTransitionKpis.set(null);
      this.stateTransitionObjectType.set('');
      this.lifecycleObjectType.set('');
      this.lifecycleSearchQuery.set('');
      this.lifecycleSearchResults.set([]);
      this.selectedLifecycleObjectId.set('');
      this.lifecycleDetail.set(null);
      this.stateAwareOcdfg.set(null);
      this.traditionalOcdfg.set(null);
      this.stateDetectionAnalysis.set(null);
      this.stateDetectionObjectType.set('');
      this.stateDetectionColorAttribute.set('__window_count');
      this.stateDetectionColorOptions.set(DEFAULT_STATE_DETECTION_COLOR_OPTIONS);
      this.stateDetectionCellDetail.set(null);
      this.stateDetectionCellTab.set('dfg');
      this.causalObjectType.set('');
      this.causalFeatureTable.set(null);
      this.causalNodes.set([]);
      this.causalEdges.set([]);
      this.causalLatentDraft.set('');
      this.causalFit.set(null);
      this.causalMessage.set('');
      this.isGeneratingCausalModel.set(false);
      this.resetGraphSettings([]);
      this.selectedIntraPatternId.set('');
      this.selectedInterPatternId.set('');
      this.activePatternTab.set('intra');
      this.activeFeature.set('statistics');
      this.fullScreenPattern.set(null);
      this.isStateDialogOpen.set(false);
    } finally {
      this.isLoading.set(false);
    }
  },
};

export type AppFileImportMethods = typeof appFileImportMethods;
