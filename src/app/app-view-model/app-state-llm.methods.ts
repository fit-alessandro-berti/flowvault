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

export const appStateLlmMethods = {
  async generateStateExpressionWithLlm(this: any): Promise<void> {
    if (!this.documentHandle) {
      return;
    }

    const config = this.currentLlmConfig();
    if (!config.apiKey.trim()) {
      this.errorMessage.set('Configure and save an LLM API key first.');
      this.openLlmConfig();
      return;
    }

    this.errorMessage.set('');
    this.stateMessage.set('');
    this.ensureLeadingObjectTypeSelection();
    this.selectedPresetId.set(LLM_STATE_PRESET_ID);
    this.isGeneratingStateExpression.set(true);

    try {
      const expression = await requestChatCompletion(config, [
        {
          role: 'system',
          content:
            'You generate Flowvault state expressions for object-centric event logs. Return only one valid expression and no markdown.',
        },
        {
          role: 'user',
          content: this.buildLlmStatePrompt(),
        },
      ]);
      this.stateQueryDraft.set(
        withLeadingObjectTypeClause(
          extractStateExpression(expression),
          this.selectedLeadingObjectType(),
        ),
      );
      this.stateMessage.set('LLM state expression generated.');
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    } finally {
      this.isGeneratingStateExpression.set(false);
    }
  },

  persistStateExpression(this: any, expression: string): void {
    const normalized = expression.trim();
    if (!normalized) {
      return;
    }

    this.persistedStateExpression.set(normalized);
    writeStoredString(STATE_EXPRESSION_STORAGE_KEY, normalized);
  },

  buildLlmStatePrompt(this: any): string {
    const summary = this.summary();
    const originalSummary = this.originalSummary();
    const stateDetection = this.stateDetectionAnalysis();
    const examples = STATE_EXPRESSION_EXAMPLES.map((example, index) => {
      return `Example ${index + 1}:\n${example}`;
    }).join('\n\n');
    const stateDetectionMetadata = stateDetection
      ? `
State Detection feature columns for ${stateDetection.object_type}:
${stateDetection.feature_columns
  .slice(0, 30)
  .map((column: string) => `- ${column}`)
  .join('\n')}
`
      : '';

    return `${this.llmStatePrompt().trim() || DEFAULT_LLM_STATE_PROMPT}

Basic OCEL metadata:
- File: ${this.fileName() || 'unknown'}
- Active events: ${summary?.events ?? 0}
- Active objects: ${summary?.objects ?? 0}
- Original events: ${originalSummary?.events ?? summary?.events ?? 0}
- Original objects: ${originalSummary?.objects ?? summary?.objects ?? 0}
- Event types: ${this.filterOptions().event_types.join(', ') || 'unknown'}
- Object types: ${this.filterOptions().object_types.join(', ') || 'unknown'}
- Active event types: ${this.selectedEventTypes().join(', ') || 'none'}
- Active object types: ${this.selectedObjectTypes().join(', ') || 'none'}
- Leading object type to use: ${this.selectedLeadingObjectType() || 'choose one from active object types'}
${stateDetectionMetadata}
State expression language:
- Shape: STATE state FOR LEADING OBJECT TYPE '<object type>' AS CASE ... END
- Each branch is WHEN <condition> THEN '<state label>'.
- Add ELSE '<state label>' unless a partial state assignment is intentional.
- Use event.type for the activity name.
- Use event.attribute_name or event."Attribute Name" for event attributes.
- Use object.attribute_name or object."Attribute Name" for the selected leading object's attributes.
- Supported comparisons include =, !=, <, <=, >, >=, LIKE, IS NULL, IS NOT NULL.
- Combine conditions with AND, OR, NOT and parentheses.
- Return concise, interpretable state labels.

Few-shot examples:
${examples}

Return only one valid Flowvault state expression.`;
  },
};

export type AppStateLlmMethods = typeof appStateLlmMethods;
