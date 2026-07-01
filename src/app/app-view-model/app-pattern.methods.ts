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

export const appPatternMethods = {
  selectIntraPattern(this: any, event: Event): void {
    this.selectedIntraPatternId.set((event.target as HTMLSelectElement).value);
  },

  selectInterPattern(this: any, event: Event): void {
    this.selectedInterPatternId.set((event.target as HTMLSelectElement).value);
  },

  summaryDisplayValue(this: any, metric: SummaryMetric): SummaryDisplayValue {
    const summary = this.summary();
    const originalSummary = this.originalSummary();
    const current = summary?.[metric] ?? 0;
    const original = originalSummary?.[metric] ?? current;

    if (!this.isFilterApplied()) {
      return {
        current: current.toLocaleString(),
        filtered: false,
      };
    }

    return {
      current: current.toLocaleString(),
      original: original.toLocaleString(),
      filtered: true,
    };
  },

  setIntraVisualization(this: any, visualization: PatternVisualization): void {
    this.intraVisualization.set(visualization);
  },

  setInterVisualization(this: any, visualization: PatternVisualization): void {
    this.interVisualization.set(visualization);
  },

  setPatternTab(this: any, tab: PatternTab): void {
    this.activePatternTab.set(tab);
  },

  togglePatternExplorer(this: any): void {
    this.isPatternExplorerOpen.update((isOpen: boolean) => !isOpen);
  },

  openFullScreenGraph(this: any, pattern: StatePattern): void {
    this.fullScreenPattern.set(pattern);
  },

  closeFullScreenGraph(this: any): void {
    this.fullScreenPattern.set(null);
  },

  applyPatternFilter(this: any, pattern: StatePattern): void {
    this.selectedPatternFilters.set([patternFilterRequest(pattern)]);
    this.applyActiveFilter();
  },

  patternOptionLabel(this: any, pattern: StatePattern): string {
    return `${pattern.support.toLocaleString()}x | ${pattern.label}`;
  },

  patternFamilyLabel(this: any, pattern: StatePattern): string {
    return pattern.family === 'inter' ? 'Inter-state' : 'Intra-state';
  },

  patternStateLabel(this: any, pattern: StatePattern): string {
    return pattern.family === 'inter'
      ? `${pattern.from_state ?? '?'} -> ${pattern.to_state ?? '?'}`
      : (pattern.state ?? '?');
  },

  topEdges(this: any, edges: StatePatternEdge[], limit = 12): StatePatternEdge[] {
    return [...edges]
      .sort((left, right) => right.weight - left.weight || left.source.localeCompare(right.source))
      .slice(0, limit);
  },

  hiddenEdgeCount(this: any, edges: StatePatternEdge[], limit = 12): number {
    return Math.max(edges.length - limit, 0);
  },

  patternGraph(this: any, pattern: StatePattern, mode: boolean | 'compact' = false): PatternGraph {
    const expanded = mode === true;
    const compact = mode === 'compact';
    const nodeWidth = compact ? 150 : expanded ? 260 : 190;
    const nodeHeight = compact ? 54 : expanded ? 92 : 68;
    const controlGap = compact ? 188 : expanded ? 330 : 238;
    const controlStartX = compact ? 52 : expanded ? 120 : 86;
    const objectStartY = compact ? 186 : expanded ? 380 : 292;
    const objectColumnGap = compact ? 188 : expanded ? 330 : 236;
    const objectRowGap = compact ? 82 : expanded ? 140 : 104;
    const width = Math.max(
      compact ? 620 : expanded ? 1320 : 960,
      controlStartX * 2 + Math.max(pattern.sequence.length - 1, 0) * controlGap + nodeWidth,
    );
    const objectColumns = Math.max(1, Math.floor((width - 120) / objectColumnGap));
    const objectRows = Math.max(1, Math.ceil(pattern.object_types.length / objectColumns));
    const height = objectStartY + objectRows * objectRowGap + 54;

    const controlNodes = pattern.sequence.map((label, index) => ({
      id: `control-${index}`,
      lines: wrapGraphLabel(
        label,
        compact ? 17 : expanded ? 31 : 22,
        compact ? 3 : expanded ? 5 : 4,
      ),
      title: label,
      x: controlStartX + index * controlGap,
      y: 52,
      kind: label.startsWith('CHANGE ') ? ('change' as const) : ('control' as const),
    }));
    const objectNodes = pattern.object_types.map((objectType, index) => ({
      id: `object-${index}`,
      lines: wrapGraphLabel(
        objectType,
        compact ? 17 : expanded ? 31 : 22,
        compact ? 3 : expanded ? 5 : 4,
      ),
      title: objectType,
      x: controlStartX + (index % objectColumns) * objectColumnGap,
      y: objectStartY + Math.floor(index / objectColumns) * objectRowGap,
      kind: 'object' as const,
    }));
    const nodes = [...controlNodes, ...objectNodes];
    const firstControlByLabel = new Map<string, PatternGraphNode>();
    const objectByType = new Map<string, PatternGraphNode>();

    for (const [index, node] of controlNodes.entries()) {
      firstControlByLabel.set(pattern.sequence[index], node);
    }
    for (const [index, objectType] of pattern.object_types.entries()) {
      objectByType.set(objectType, objectNodes[index]);
    }

    const edges: PatternGraphEdge[] = [];
    for (let index = 0; index < controlNodes.length - 1; index += 1) {
      const source = controlNodes[index];
      const target = controlNodes[index + 1];
      const weight =
        pattern.df_edges.find(
          (edge) =>
            edge.source === pattern.sequence[index] && edge.target === pattern.sequence[index + 1],
        )?.weight ?? 1;
      edges.push({
        id: `df-${index}`,
        x1: source.x + nodeWidth,
        y1: source.y + nodeHeight / 2,
        x2: target.x,
        y2: target.y + nodeHeight / 2,
        label: weight.toLocaleString(),
        kind: 'df',
      });
    }

    for (const [index, edge] of pattern.eo_edges.entries()) {
      const source = firstControlByLabel.get(edge.source);
      const target = objectByType.get(edge.target);
      if (!source || !target) {
        continue;
      }
      edges.push({
        id: `eo-${index}`,
        x1: source.x + nodeWidth / 2,
        y1: source.y + nodeHeight,
        x2: target.x + nodeWidth / 2,
        y2: target.y,
        label: edge.weight.toLocaleString(),
        kind: 'eo',
      });
    }

    for (const [index, edge] of pattern.oo_edges.entries()) {
      const source = objectByType.get(edge.source);
      const target = objectByType.get(edge.target);
      if (!source || !target || source === target) {
        continue;
      }
      edges.push({
        id: `oo-${index}`,
        x1: source.x + nodeWidth / 2,
        y1: source.y + nodeHeight / 2,
        x2: target.x + nodeWidth / 2,
        y2: target.y + nodeHeight / 2,
        label: edge.weight.toLocaleString(),
        kind: 'oo',
      });
    }

    return { width, height, nodeWidth, nodeHeight, nodes, edges };
  },

  loadStatePatterns(this: any, preserveSelection = false): void {
    if (!this.documentHandle) {
      this.patternAnalysis.set(null);
      this.stateAwareOcdfg.set(null);
      return;
    }

    const previousIntraId = this.selectedIntraPatternId();
    const previousInterId = this.selectedInterPatternId();
    const previousFullScreenPatternId = this.fullScreenPattern()?.id;
    const analysis = JSON.parse(this.documentHandle.statePatternsJson()) as StatePatternAnalysis;
    this.patternAnalysis.set(analysis);
    this.loadStateAwareOcdfg();
    if (!preserveSelection) {
      this.activePatternTab.set('intra');
    }
    this.selectedIntraPatternId.set(
      preserveSelection
        ? (selectedPattern(analysis.intra, previousIntraId)?.id ?? '')
        : (analysis.intra[0]?.id ?? ''),
    );
    this.selectedInterPatternId.set(
      preserveSelection
        ? (selectedPattern(analysis.inter, previousInterId)?.id ?? '')
        : (analysis.inter[0]?.id ?? ''),
    );

    if (previousFullScreenPatternId) {
      this.fullScreenPattern.set(
        [...analysis.intra, ...analysis.inter].find(
          (pattern) => pattern.id === previousFullScreenPatternId,
        ) ?? null,
      );
    }
  },
};

export type AppPatternMethods = typeof appPatternMethods;
