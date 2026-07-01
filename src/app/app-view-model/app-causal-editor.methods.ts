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

export const appCausalEditorMethods = {
  onCausalObjectTypeChange(this: any, event: Event): void {
    this.causalObjectType.set((event.target as HTMLSelectElement).value);
    this.resetCausalModel();
    this.loadCausalFeatureTable();
  },

  reloadCausalFeatureTable(this: any): void {
    this.loadCausalFeatureTable();
  },

  downloadCausalFeatureTable(this: any): void {
    if (!this.documentHandle) {
      return;
    }

    try {
      this.ensureCausalObjectType();
      if (!this.causalObjectType()) {
        return;
      }
      const csv = this.documentHandle.causalFeatureTableCsv(this.causalFeatureTableRequestJson());
      this.downloadNamed(
        csv,
        'text/csv',
        `${exportBaseName(this.fileName())}-${safeFilePart(this.causalObjectType())}-causal-features.csv`,
      );
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  },

  causalPreviewColumns(this: any, table: CausalFeatureTableResult): string[] {
    return table.feature_columns;
  },

  causalPreviewRows(this: any, table: CausalFeatureTableResult): StateDetectionPreviewRow[] {
    return table.table_preview.slice(0, 10);
  },

  causalPreviewValues(this: any, row: StateDetectionPreviewRow): number[] {
    return row.values;
  },

  onCausalFeatureDragStart(this: any, event: DragEvent, feature: string): void {
    event.dataTransfer?.setData('text/plain', feature);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'copy';
    }
  },

  allowCausalDrop(this: any, event: DragEvent): void {
    event.preventDefault();
  },

  dropCausalFeature(this: any, event: DragEvent, role: 'observable' | 'outcome'): void {
    event.preventDefault();
    const feature = event.dataTransfer?.getData('text/plain') ?? '';
    if (!feature) {
      return;
    }
    this.addCausalFeatureNode(role, feature);
  },

  addCausalFeatureNode(this: any, role: 'observable' | 'outcome', feature: string): void {
    const node: CausalModelNode = {
      id: nextCausalNodeId(role, this.causalNodes()),
      label: causalFeatureLabel(feature),
      role,
      feature,
      operation: 'identity',
    };
    this.causalNodes.set([...this.causalNodes(), node]);
    this.causalFit.set(null);
    this.causalMessage.set(`${role === 'observable' ? 'Observable' : 'Outcome'} added.`);
  },

  onCausalNodeLabelChange(this: any, nodeId: string, event: Event): void {
    const label = (event.target as HTMLInputElement).value;
    this.causalNodes.set(
      this.causalNodes().map((node: CausalModelNode) => (node.id === nodeId ? { ...node, label } : node)),
    );
    this.causalFit.set(null);
  },

  onCausalNodeRoleChange(this: any, nodeId: string, event: Event): void {
    const role = (event.target as HTMLSelectElement).value as 'observable' | 'outcome';
    const nodes = this.causalNodes().map((node: CausalModelNode) =>
      node.id === nodeId && node.role !== 'latent' ? { ...node, role } : node,
    );
    this.causalNodes.set(nodes);
    this.causalEdges.set(pruneCausalEdges(nodes, this.causalEdges()));
    this.causalFit.set(null);
  },

  onCausalOperationChange(this: any, nodeId: string, event: Event): void {
    const operation = (event.target as HTMLSelectElement).value as CausalOperation;
    this.causalNodes.set(
      this.causalNodes().map((node: CausalModelNode) => (node.id === nodeId ? { ...node, operation } : node)),
    );
    this.causalFit.set(null);
  },

  onCausalLatentDraftChange(this: any, event: Event): void {
    this.causalLatentDraft.set((event.target as HTMLInputElement).value);
  },

  addCausalLatent(this: any): void {
    const label =
      this.causalLatentDraft().trim() || `Latent ${this.causalLatentNodes().length + 1}`;
    const node: CausalModelNode = {
      id: nextCausalNodeId('latent', this.causalNodes()),
      label,
      role: 'latent',
      operation: 'identity',
    };
    this.causalNodes.set([...this.causalNodes(), node]);
    this.causalLatentDraft.set('');
    this.causalFit.set(null);
    this.causalMessage.set('Latent variable added.');
  },

  removeCausalNode(this: any, nodeId: string): void {
    this.causalNodes.set(this.causalNodes().filter((node: CausalModelNode) => node.id !== nodeId));
    this.causalEdges.set(
      this.causalEdges().filter((edge: { source: string; target: string }) => edge.source !== nodeId && edge.target !== nodeId),
    );
    this.causalFit.set(null);
  },

  isCausalEdgeSelected(this: any, source: string, target: string): boolean {
    return this.causalEdges().some((edge: { source: string; target: string }) => edge.source === source && edge.target === target);
  },

  isCausalEdgeDisabled(this: any, source: string, target: string): boolean {
    if (this.isCausalEdgeSelected(source, target)) {
      return false;
    }
    return !canAddCausalEdge(this.causalNodes(), this.causalEdges(), source, target);
  },

  toggleCausalEdge(this: any, source: string, target: string, event: Event): void {
    const checked = (event.target as HTMLInputElement).checked;
    if (!checked) {
      this.causalEdges.set(
        this.causalEdges().filter((edge: { source: string; target: string }) => edge.source !== source || edge.target !== target),
      );
      this.causalFit.set(null);
      return;
    }

    if (!canAddCausalEdge(this.causalNodes(), this.causalEdges(), source, target)) {
      (event.target as HTMLInputElement).checked = false;
      this.causalMessage.set('That edge would violate the DAG or role constraints.');
      return;
    }

    this.causalEdges.set([...this.causalEdges(), { source, target }]);
    this.causalFit.set(null);
  },
};

export type AppCausalEditorMethods = typeof appCausalEditorMethods;
