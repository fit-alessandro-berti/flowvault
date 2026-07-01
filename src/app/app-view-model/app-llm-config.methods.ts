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

export const appLlmConfigMethods = {
  openLlmConfig(this: any): void {
    this.llmStatus.set('');
    this.isExportMenuOpen.set(false);
    this.isFilterMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.isLlmConfigOpen.set(true);
  },

  closeLlmConfig(this: any): void {
    this.isLlmConfigOpen.set(false);
  },

  onLlmProviderChange(this: any, event: Event): void {
    const provider = providerById((event.target as HTMLSelectElement).value);
    this.llmProvider.set(provider.id);
    this.llmModel.set(provider.defaultModel);
    this.llmStatus.set('');
  },

  onLlmModelChange(this: any, event: Event): void {
    this.llmModel.set((event.target as HTMLInputElement).value);
    this.llmStatus.set('');
  },

  onLlmApiKeyChange(this: any, event: Event): void {
    this.llmApiKey.set((event.target as HTMLInputElement).value);
    this.llmStatus.set('');
  },

  saveLlmConfig(this: any): void {
    writeStoredJson(LLM_CONFIG_STORAGE_KEY, this.currentLlmConfig());
    this.llmStatus.set('Configuration saved.');
  },

  async testLlmConfig(this: any): Promise<void> {
    this.llmStatus.set('');
    if (!this.currentLlmConfig().apiKey.trim()) {
      this.llmStatus.set('API key is required.');
      return;
    }

    this.isTestingLlm.set(true);
    try {
      const response = await requestChatCompletion(this.currentLlmConfig(), [
        {
          role: 'system',
          content: 'Respond with OK.',
        },
        {
          role: 'user',
          content: 'Connection test.',
        },
      ]);
      this.llmStatus.set(`Test succeeded: ${response.slice(0, 80)}`);
    } catch (error) {
      this.llmStatus.set(errorToMessage(error));
    } finally {
      this.isTestingLlm.set(false);
    }
  },

  currentLlmConfig(this: any): LlmConfig {
    const provider = providerById(this.llmProvider());
    return {
      provider: provider.id,
      model: this.llmModel().trim() || provider.defaultModel,
      apiKey: this.llmApiKey().trim(),
    };
  },
};

export type AppLlmConfigMethods = typeof appLlmConfigMethods;
