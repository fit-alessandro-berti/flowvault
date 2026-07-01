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

export const appCausalAnalysisMethods = {
  fitCausalModel(this: any): void {
    if (!this.documentHandle || !this.canFitCausalModel()) {
      return;
    }

    try {
      const fit = JSON.parse(
        this.documentHandle.fitCausalModelJson(this.causalModelRequestJson()),
      ) as CausalFitResult;
      this.causalFit.set(fit);
      this.causalMessage.set(
        `Fitted ${fit.edges.length.toLocaleString()} edges over ${fit.sample_count.toLocaleString()} objects.`,
      );
      this.errorMessage.set('');
    } catch (error) {
      this.causalFit.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  },

  async generateCausalModelWithLlm(this: any): Promise<void> {
    if (!this.documentHandle) {
      return;
    }

    const config = this.currentLlmConfig();
    if (!config.apiKey.trim()) {
      this.errorMessage.set('Configure and save an LLM API key first.');
      this.openLlmConfig();
      return;
    }

    if (!this.causalFeatureTable()) {
      this.loadCausalFeatureTable();
    }
    const table = this.causalFeatureTable();
    if (!table || table.feature_columns.length === 0) {
      this.errorMessage.set('Load causal features before asking the LLM.');
      return;
    }

    this.errorMessage.set('');
    this.causalMessage.set('');
    this.isGeneratingCausalModel.set(true);
    try {
      const response = await requestChatCompletion(config, [
        {
          role: 'system',
          content:
            'You generate Flowvault causal model JSON. Return only valid JSON and no markdown.',
        },
        {
          role: 'user',
          content: this.buildLlmCausalPrompt(table),
        },
      ]);
      const suggestion = parseCausalModelSuggestion(response, table.feature_columns);
      this.causalNodes.set(suggestion.nodes);
      this.causalEdges.set(pruneCausalEdges(suggestion.nodes, suggestion.edges));
      this.causalFit.set(null);
      this.causalMessage.set(
        `LLM suggested ${suggestion.nodes.length.toLocaleString()} nodes and ${this.causalEdges().length.toLocaleString()} DAG edges.`,
      );
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    } finally {
      this.isGeneratingCausalModel.set(false);
    }
  },

  loadCausalFeatureTable(this: any): void {
    if (!this.documentHandle) {
      this.causalFeatureTable.set(null);
      return;
    }

    try {
      this.ensureCausalObjectType();
      if (!this.causalObjectType()) {
        this.causalFeatureTable.set(null);
        return;
      }
      const table = JSON.parse(
        this.documentHandle.causalFeatureTableJson(this.causalFeatureTableRequestJson()),
      ) as CausalFeatureTableResult;
      this.causalFeatureTable.set(table);
      this.causalNodes.set(
        this.causalNodes().filter(
          (node: CausalModelNode) => node.role === 'latent' || table.feature_columns.includes(node.feature ?? ''),
        ),
      );
      this.causalEdges.set(pruneCausalEdges(this.causalNodes(), this.causalEdges()));
      this.causalFit.set(null);
      this.causalMessage.set(
        `Loaded ${table.feature_count.toLocaleString()} features for ${table.object_count.toLocaleString()} objects.`,
      );
      this.errorMessage.set('');
    } catch (error) {
      this.causalFeatureTable.set(null);
      this.causalFit.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  },

  ensureCausalObjectType(this: any): void {
    const selected = this.selectedObjectTypes();
    const current = this.causalObjectType();
    if (current && selected.includes(current)) {
      return;
    }
    this.causalObjectType.set(selected[0] ?? this.filterOptions().object_types[0] ?? '');
  },

  resetCausalModel(this: any): void {
    this.causalFeatureTable.set(null);
    this.causalNodes.set([]);
    this.causalEdges.set([]);
    this.causalLatentDraft.set('');
    this.causalFit.set(null);
    this.causalMessage.set('');
  },

  causalFeatureTableRequestJson(this: any): string {
    return JSON.stringify({ object_type: this.causalObjectType() });
  },

  causalModelRequestJson(this: any): string {
    return JSON.stringify({
      object_type: this.causalObjectType(),
      nodes: this.causalNodes(),
      edges: this.causalEdges(),
    });
  },

  buildLlmCausalPrompt(this: any, table: CausalFeatureTableResult): string {
    const summary = this.summary();
    const features = table.feature_columns
      .slice(0, 160)
      .map((feature) => `- ${feature}`)
      .join('\n');
    const omitted = Math.max(table.feature_columns.length - 160, 0);

    return `Create a small DAG causal model for Flowvault.

OCEL metadata:
- File: ${this.fileName() || 'unknown'}
- Active events: ${summary?.events ?? 0}
- Active objects: ${summary?.objects ?? 0}
- Causal object type: ${table.object_type}
- Rows in feature table: ${table.object_count}
- Feature columns: ${table.feature_count}

Available feature columns:
${features}
${omitted > 0 ? `- ... ${omitted} additional columns omitted from the prompt` : ''}

Causal model JSON schema:
{
  "nodes": [
    {"id":"obs_1","label":"Readable name","role":"observable","feature":"exact feature column","operation":"identity"},
    {"id":"lat_1","label":"Latent concept","role":"latent"},
    {"id":"out_1","label":"Readable outcome","role":"outcome","feature":"exact feature column","operation":"identity"}
  ],
  "edges": [
    {"source":"obs_1","target":"lat_1"},
    {"source":"lat_1","target":"out_1"}
  ]
}

Rules:
- Return only JSON.
- Use only exact feature column names from the list.
- Valid roles are observable, latent, outcome.
- Valid operations are identity, log10, log_e, sqrt.
- Legal edges are observable -> latent, latent -> latent, and latent -> outcome.
- The graph must be a DAG.
- The same feature column may appear in multiple observable/outcome nodes.
- Prefer 2-5 observables, 1-3 latents, and 1-3 outcomes.`;
  },
};

export type AppCausalAnalysisMethods = typeof appCausalAnalysisMethods;
