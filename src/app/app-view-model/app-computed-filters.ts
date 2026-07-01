import { computed } from '@angular/core';
import { presetsForFile } from '../state-query-presets';
import type { DfEdgeOption, AppliedFilterChip } from '../models/filter.models';
import type { PatternExplorerRow } from '../models/pattern.models';
import type { SummaryCard } from '../models/summary.models';
import { edgeKey, edgeLabel, emptySummaryValue, filterDescription, selectedPattern } from '../helpers/common.helpers';
import { LLM_STATE_PRESET_ID } from '../helpers/state-expression.helpers';
import { patternFilterLabel } from '../helpers/pattern.helpers';
import { timeRangeLabel } from '../helpers/time-range.helpers';
import { AppStateFields } from './app-state-fields';

export class AppFilterComputedState extends AppStateFields {
  protected readonly hasDocument = computed(() => this.summary() !== null);
  protected readonly hasAppliedState = computed(
    () => (this.originalSummary()?.stateful_events ?? this.summary()?.stateful_events ?? 0) > 0,
  );
  protected readonly isFilterApplied = computed(
    () =>
      this.selectedEventTypes().length !== this.filterOptions().event_types.length ||
      this.selectedObjectTypes().length !== this.filterOptions().object_types.length ||
      this.selectedDfNodes().length > 0 ||
      this.selectedDfEdges().length > 0 ||
      this.selectedTimeRange() !== null ||
      this.selectedTextAttribute() !== null ||
      this.selectedPatternFilters().length > 0,
  );
  protected readonly stateQueryPresets = computed(() => presetsForFile(this.fileName()));
  protected readonly isLlmStateMode = computed(
    () => this.selectedPresetId() === LLM_STATE_PRESET_ID,
  );
  protected readonly leadingObjectTypeOptions = computed(() => {
    const selected = this.selectedObjectTypes();
    return selected.length > 0 ? selected : this.filterOptions().object_types;
  });
  protected readonly appliedFilters = computed<AppliedFilterChip[]>(() => {
    const options = this.filterOptions();
    const chips: AppliedFilterChip[] = [];

    if (this.selectedEventTypes().length < options.event_types.length) {
      chips.push({
        kind: 'activities',
        label: `Activities ${this.selectedEventTypes().length}/${options.event_types.length}`,
        description: filterDescription('Selected activities', this.selectedEventTypes()),
        removeLabel: 'Remove activity filter',
      });
    }

    if (this.selectedObjectTypes().length < options.object_types.length) {
      chips.push({
        kind: 'objectTypes',
        label: `Object types ${this.selectedObjectTypes().length}/${options.object_types.length}`,
        description: filterDescription('Selected object types', this.selectedObjectTypes()),
        removeLabel: 'Remove object type filter',
      });
    }

    if (this.selectedDfNodes().length > 0) {
      chips.push({
        kind: 'dfNodes',
        label: `OC-DFG nodes ${this.selectedDfNodes().length}`,
        description: filterDescription('Objects containing activities', this.selectedDfNodes()),
        removeLabel: 'Remove OC-DFG node filter',
      });
    }

    if (this.selectedDfEdges().length > 0) {
      const labels = this.selectedDfEdges().map((edge) => edgeLabel(edge));
      chips.push({
        kind: 'dfEdges',
        label: `OC-DFG edges ${this.selectedDfEdges().length}`,
        description: filterDescription('Objects containing directly-follows edges', labels),
        removeLabel: 'Remove OC-DFG edge filter',
      });
    }

    const timeRange = this.selectedTimeRange();
    if (timeRange) {
      chips.push({
        kind: 'timeframe',
        label: 'Timeframe',
        description: timeRangeLabel(timeRange),
        removeLabel: 'Remove timeframe filter',
      });
    }

    const textAttribute = this.selectedTextAttribute();
    if (textAttribute && textAttribute.values.length > 0) {
      chips.push({
        kind: 'textAttributes',
        label: `${textAttribute.name} ${textAttribute.values.length}`,
        description: filterDescription(
          `${textAttribute.scope} attribute ${textAttribute.name}`,
          textAttribute.values,
        ),
        removeLabel: 'Remove text attribute filter',
      });
    }

    if (this.selectedPatternFilters().length > 0) {
      chips.push({
        kind: 'patterns',
        label: `Patterns ${this.selectedPatternFilters().length}`,
        description: filterDescription(
          'Objects matching selected state patterns',
          this.selectedPatternFilters().map(patternFilterLabel),
        ),
        removeLabel: 'Remove pattern filter',
      });
    }

    return chips;
  });
  protected readonly dfEdgeOptions = computed<DfEdgeOption[]>(() => {
    const graph = this.traditionalOcdfg();
    if (!graph) {
      return [];
    }
    const nodeById = new Map(graph.nodes.map((node) => [node.id, node]));
    const seen = new Set<string>();
    const options: DfEdgeOption[] = [];
    for (const edge of graph.edges) {
      const source = nodeById.get(edge.source);
      const target = nodeById.get(edge.target);
      if (!source || !target || source.kind !== 'activity' || target.kind !== 'activity') {
        continue;
      }
      const option = {
        source: source.label,
        target: target.label,
        label: `${source.label} -> ${target.label}`,
      };
      const key = edgeKey(option);
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      options.push(option);
    }
    return options;
  });
  protected readonly intraPatterns = computed(() => this.patternAnalysis()?.intra ?? []);
  protected readonly interPatterns = computed(() => this.patternAnalysis()?.inter ?? []);
  protected readonly selectedIntraPattern = computed(() =>
    selectedPattern(this.intraPatterns(), this.selectedIntraPatternId()),
  );
  protected readonly selectedInterPattern = computed(() =>
    selectedPattern(this.interPatterns(), this.selectedInterPatternId()),
  );
  protected readonly patternExplorerRows = computed<PatternExplorerRow[]>(() =>
    [...this.intraPatterns(), ...this.interPatterns()]
      .sort(
        (left, right) =>
          right.support - left.support ||
          right.mass - left.mass ||
          left.label.localeCompare(right.label),
      )
      .map((pattern) => ({
        pattern,
        graph: (this as any).patternGraph(pattern, 'compact'),
      })),
  );
  protected readonly summaryCards = computed<SummaryCard[]>(() => {
    const summary = this.summary();

    return [
      {
        label: 'Events',
        value: summary ? (this as any).summaryDisplayValue('events') : emptySummaryValue(),
      },
      {
        label: 'Objects',
        value: summary ? (this as any).summaryDisplayValue('objects') : emptySummaryValue(),
      },
      {
        label: 'E2O',
        value: summary ? (this as any).summaryDisplayValue('e2o_relationships') : emptySummaryValue(),
      },
      {
        label: 'O2O',
        value: summary ? (this as any).summaryDisplayValue('o2o_relationships') : emptySummaryValue(),
      },
    ];
  });
}
