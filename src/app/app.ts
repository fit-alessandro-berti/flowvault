import { Component, computed, inject, signal } from '@angular/core';
import { exportBaseName, formatHintForFile } from './ocel-file';
import { presetsForFile, StateQueryPreset } from './state-query-presets';
import { ProcessGraphComponent } from './process-graph.component';
import {
  OcelDocumentHandle,
  OcelFilterOptions,
  ProcessGraph,
  StatePattern,
  StatePatternAnalysis,
  StatePatternEdge,
  OcelSummary,
  OcelWasmService,
  StateQueryResult,
} from './ocel-wasm.service';

interface SummaryCard {
  label: string;
  value: SummaryDisplayValue;
}

interface SummaryDisplayValue {
  current: string;
  original?: string;
  filtered: boolean;
}

type SummaryMetric = keyof Pick<
  OcelSummary,
  | 'event_types'
  | 'object_types'
  | 'events'
  | 'objects'
  | 'e2o_relationships'
  | 'o2o_relationships'
  | 'interned_strings'
  | 'objects_with_lifecycle'
  | 'stateful_events'
>;

interface FilterRequest {
  event_types: string[];
  object_types: string[];
}

type FilterDialogKind = 'activities' | 'objectTypes';
type PatternTab = 'intra' | 'inter';
type PatternVisualization = 'text' | 'graph';

interface AppliedFilterChip {
  kind: FilterDialogKind;
  label: string;
  description: string;
  removeLabel: string;
}

interface PatternGraphNode {
  id: string;
  lines: string[];
  title: string;
  x: number;
  y: number;
  kind: 'control' | 'change' | 'object';
}

interface PatternGraphEdge {
  id: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  label: string;
  kind: 'df' | 'eo' | 'oo';
}

interface PatternGraph {
  width: number;
  height: number;
  nodeWidth: number;
  nodeHeight: number;
  nodes: PatternGraphNode[];
  edges: PatternGraphEdge[];
}

@Component({
  selector: 'app-root',
  imports: [ProcessGraphComponent],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App {
  private readonly ocelWasm = inject(OcelWasmService);
  private documentHandle?: OcelDocumentHandle;

  protected readonly isDragging = signal(false);
  protected readonly isLoading = signal(false);
  protected readonly fileName = signal('');
  protected readonly errorMessage = signal('');
  protected readonly stateMessage = signal('');
  protected readonly isStateDialogOpen = signal(false);
  protected readonly selectedPresetId = signal('');
  protected readonly selectedLeadingObjectType = signal('');
  protected readonly stateQueryDraft = signal('');
  protected readonly summary = signal<OcelSummary | null>(null);
  protected readonly originalSummary = signal<OcelSummary | null>(null);
  protected readonly filterOptions = signal<OcelFilterOptions>({
    event_types: [],
    object_types: [],
  });
  protected readonly selectedEventTypes = signal<string[]>([]);
  protected readonly selectedObjectTypes = signal<string[]>([]);
  protected readonly draftEventTypes = signal<string[]>([]);
  protected readonly draftObjectTypes = signal<string[]>([]);
  protected readonly filterDialog = signal<FilterDialogKind | null>(null);
  protected readonly patternAnalysis = signal<StatePatternAnalysis | null>(null);
  protected readonly stateAwareOcdfg = signal<ProcessGraph | null>(null);
  protected readonly selectedIntraPatternId = signal('');
  protected readonly selectedInterPatternId = signal('');
  protected readonly activePatternTab = signal<PatternTab>('intra');
  protected readonly intraVisualization = signal<PatternVisualization>('text');
  protected readonly interVisualization = signal<PatternVisualization>('text');
  protected readonly fullScreenPattern = signal<StatePattern | null>(null);
  protected readonly hasDocument = computed(() => this.summary() !== null);
  protected readonly isFilterApplied = computed(
    () =>
      this.selectedEventTypes().length !== this.filterOptions().event_types.length ||
      this.selectedObjectTypes().length !== this.filterOptions().object_types.length,
  );
  protected readonly stateQueryPresets = computed(() => presetsForFile(this.fileName()));
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

    return chips;
  });
  protected readonly intraPatterns = computed(() => this.patternAnalysis()?.intra ?? []);
  protected readonly interPatterns = computed(() => this.patternAnalysis()?.inter ?? []);
  protected readonly selectedIntraPattern = computed(() =>
    selectedPattern(this.intraPatterns(), this.selectedIntraPatternId()),
  );
  protected readonly selectedInterPattern = computed(() =>
    selectedPattern(this.interPatterns(), this.selectedInterPatternId()),
  );
  protected readonly summaryCards = computed<SummaryCard[]>(() => {
    const summary = this.summary();

    return [
      {
        label: 'Events',
        value: summary ? this.summaryDisplayValue('events') : emptySummaryValue(),
      },
      {
        label: 'Objects',
        value: summary ? this.summaryDisplayValue('objects') : emptySummaryValue(),
      },
      {
        label: 'E2O',
        value: summary ? this.summaryDisplayValue('e2o_relationships') : emptySummaryValue(),
      },
      {
        label: 'O2O',
        value: summary ? this.summaryDisplayValue('o2o_relationships') : emptySummaryValue(),
      },
    ];
  });

  async onFileSelected(event: Event): Promise<void> {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';

    if (file) {
      await this.importFile(file);
    }
  }

  onDragOver(event: DragEvent): void {
    event.preventDefault();
    this.isDragging.set(true);
  }

  onDragLeave(event: DragEvent): void {
    if (event.currentTarget === event.target) {
      this.isDragging.set(false);
    }
  }

  async onDrop(event: DragEvent): Promise<void> {
    event.preventDefault();
    this.isDragging.set(false);

    const file = event.dataTransfer?.files?.[0];
    if (file) {
      await this.importFile(file);
    }
  }

  exportJson(): void {
    this.exportDocument('json');
  }

  exportXml(): void {
    this.exportDocument('xml');
  }

  openStateDialog(): void {
    if (!this.documentHandle) {
      return;
    }

    const presets = this.stateQueryPresets();
    const selectedPreset =
      presets.find((preset) => preset.id === this.selectedPresetId()) ?? presets[0];

    if (selectedPreset) {
      this.selectStatePreset(selectedPreset);
    } else {
      this.ensureLeadingObjectTypeSelection();
    }

    this.errorMessage.set('');
    this.isStateDialogOpen.set(true);
  }

  closeStateDialog(): void {
    this.isStateDialogOpen.set(false);
  }

  selectStatePreset(preset: StateQueryPreset): void {
    const leadingObjectType = this.validLeadingObjectType(preset.leadingObjectType);
    this.selectedPresetId.set(preset.id);
    this.selectedLeadingObjectType.set(leadingObjectType);
    this.stateQueryDraft.set(withLeadingObjectTypeClause(preset.query, leadingObjectType));
  }

  onStateQueryDraftChange(event: Event): void {
    this.stateQueryDraft.set((event.target as HTMLTextAreaElement).value);
  }

  onLeadingObjectTypeChange(event: Event): void {
    const leadingObjectType = (event.target as HTMLSelectElement).value;
    this.selectedLeadingObjectType.set(leadingObjectType);
    this.stateQueryDraft.set(
      withLeadingObjectTypeClause(this.stateQueryDraft(), leadingObjectType),
    );
  }

  applyStateQuery(): void {
    if (!this.documentHandle) {
      return;
    }

    this.errorMessage.set('');
    this.stateMessage.set('');
    this.ensureLeadingObjectTypeSelection();
    const query = withLeadingObjectTypeClause(
      this.stateQueryDraft(),
      this.selectedLeadingObjectType(),
    );
    this.stateQueryDraft.set(query);

    try {
      const result = JSON.parse(this.documentHandle.applyStateQuery(query)) as StateQueryResult;
      this.summary.set(JSON.parse(this.documentHandle.summaryJson()) as OcelSummary);
      this.originalSummary.set(
        JSON.parse(this.documentHandle.originalSummaryJson()) as OcelSummary,
      );
      this.loadStatePatterns();
      this.stateMessage.set(
        `Added ${result.attribute} for ${result.leading_object_type} to ${result.assigned_events.toLocaleString()} of ${result.total_events.toLocaleString()} events.`,
      );
      this.isStateDialogOpen.set(false);
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  }

  private async importFile(file: File): Promise<void> {
    this.isLoading.set(true);
    this.errorMessage.set('');

    try {
      const text = await file.text();
      const imported = await this.ocelWasm.importDocument(text, formatHintForFile(file.name));

      this.documentHandle?.free();
      this.documentHandle = imported.document;
      this.fileName.set(file.name);
      this.summary.set(imported.summary);
      this.originalSummary.set(imported.originalSummary);
      this.filterOptions.set(imported.filterOptions);
      this.selectedEventTypes.set(imported.filterOptions.event_types);
      this.selectedObjectTypes.set(imported.filterOptions.object_types);
      this.draftEventTypes.set(imported.filterOptions.event_types);
      this.draftObjectTypes.set(imported.filterOptions.object_types);
      this.filterDialog.set(null);
      this.stateMessage.set('');
      this.patternAnalysis.set(null);
      this.stateAwareOcdfg.set(null);
      this.selectedIntraPatternId.set('');
      this.selectedInterPatternId.set('');
      this.activePatternTab.set('intra');
      this.fullScreenPattern.set(null);
      this.isStateDialogOpen.set(false);
      this.initializeStatePresetForFile(file.name);
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
      this.summary.set(null);
      this.originalSummary.set(null);
      this.filterOptions.set({ event_types: [], object_types: [] });
      this.selectedEventTypes.set([]);
      this.selectedObjectTypes.set([]);
      this.draftEventTypes.set([]);
      this.draftObjectTypes.set([]);
      this.filterDialog.set(null);
      this.selectedLeadingObjectType.set('');
      this.fileName.set(file.name);
      this.documentHandle?.free();
      this.documentHandle = undefined;
      this.stateMessage.set('');
      this.patternAnalysis.set(null);
      this.stateAwareOcdfg.set(null);
      this.selectedIntraPatternId.set('');
      this.selectedInterPatternId.set('');
      this.activePatternTab.set('intra');
      this.fullScreenPattern.set(null);
      this.isStateDialogOpen.set(false);
    } finally {
      this.isLoading.set(false);
    }
  }

  private exportDocument(format: 'json' | 'xml'): void {
    if (!this.documentHandle) {
      return;
    }

    try {
      const content =
        format === 'json' ? this.documentHandle.exportJson() : this.documentHandle.exportXml();
      const mimeType = format === 'json' ? 'application/json' : 'application/xml';
      this.download(content, mimeType, format);
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  }

  private download(content: string, mimeType: string, extension: 'json' | 'xml'): void {
    const blob = new Blob([content], { type: `${mimeType};charset=utf-8` });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');

    anchor.href = url;
    anchor.download = `${exportBaseName(this.fileName())}.${extension}`;
    anchor.click();
    URL.revokeObjectURL(url);
  }

  private initializeStatePresetForFile(fileName: string): void {
    const preset = presetsForFile(fileName)[0];
    this.selectStatePreset(preset);
  }

  private ensureLeadingObjectTypeSelection(): void {
    this.selectedLeadingObjectType.set(
      this.validLeadingObjectType(this.selectedLeadingObjectType()),
    );
  }

  private validLeadingObjectType(candidate: string): string {
    const options = this.leadingObjectTypeOptions();
    if (candidate && options.includes(candidate)) {
      return candidate;
    }
    return options[0] ?? this.filterOptions().object_types[0] ?? candidate;
  }

  protected selectIntraPattern(event: Event): void {
    this.selectedIntraPatternId.set((event.target as HTMLSelectElement).value);
  }

  protected selectInterPattern(event: Event): void {
    this.selectedInterPatternId.set((event.target as HTMLSelectElement).value);
  }

  protected openActivityFilterDialog(): void {
    this.draftEventTypes.set([...this.selectedEventTypes()]);
    this.filterDialog.set('activities');
  }

  protected openObjectTypeFilterDialog(): void {
    this.draftObjectTypes.set([...this.selectedObjectTypes()]);
    this.filterDialog.set('objectTypes');
  }

  protected closeFilterDialog(): void {
    this.filterDialog.set(null);
  }

  protected toggleDraftEventType(eventType: string, event: Event): void {
    this.draftEventTypes.set(
      toggleSelection(
        this.draftEventTypes(),
        eventType,
        (event.target as HTMLInputElement).checked,
      ),
    );
  }

  protected toggleDraftObjectType(objectType: string, event: Event): void {
    this.draftObjectTypes.set(
      toggleSelection(
        this.draftObjectTypes(),
        objectType,
        (event.target as HTMLInputElement).checked,
      ),
    );
  }

  protected selectAllDraftEventTypes(): void {
    this.draftEventTypes.set([...this.filterOptions().event_types]);
  }

  protected clearDraftEventTypes(): void {
    this.draftEventTypes.set([]);
  }

  protected selectAllDraftObjectTypes(): void {
    this.draftObjectTypes.set([...this.filterOptions().object_types]);
  }

  protected clearDraftObjectTypes(): void {
    this.draftObjectTypes.set([]);
  }

  protected applyFilterDialog(): void {
    const dialog = this.filterDialog();

    if (dialog === 'activities') {
      this.selectedEventTypes.set([...this.draftEventTypes()]);
    }
    if (dialog === 'objectTypes') {
      this.selectedObjectTypes.set([...this.draftObjectTypes()]);
    }

    this.filterDialog.set(null);
    this.applyActiveFilter();
  }

  protected removeFilter(kind: FilterDialogKind): void {
    if (kind === 'activities') {
      this.selectedEventTypes.set([...this.filterOptions().event_types]);
      this.draftEventTypes.set([...this.filterOptions().event_types]);
    } else {
      this.selectedObjectTypes.set([...this.filterOptions().object_types]);
      this.draftObjectTypes.set([...this.filterOptions().object_types]);
    }

    this.filterDialog.set(null);
    this.applyActiveFilter();
  }

  protected summaryDisplayValue(metric: SummaryMetric): SummaryDisplayValue {
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
  }

  protected setIntraVisualization(visualization: PatternVisualization): void {
    this.intraVisualization.set(visualization);
  }

  protected setInterVisualization(visualization: PatternVisualization): void {
    this.interVisualization.set(visualization);
  }

  protected setPatternTab(tab: PatternTab): void {
    this.activePatternTab.set(tab);
  }

  protected openFullScreenGraph(pattern: StatePattern): void {
    this.fullScreenPattern.set(pattern);
  }

  protected closeFullScreenGraph(): void {
    this.fullScreenPattern.set(null);
  }

  protected patternOptionLabel(pattern: StatePattern): string {
    return `${pattern.support.toLocaleString()}x | ${pattern.label}`;
  }

  protected topEdges(edges: StatePatternEdge[], limit = 12): StatePatternEdge[] {
    return [...edges]
      .sort((left, right) => right.weight - left.weight || left.source.localeCompare(right.source))
      .slice(0, limit);
  }

  protected hiddenEdgeCount(edges: StatePatternEdge[], limit = 12): number {
    return Math.max(edges.length - limit, 0);
  }

  protected patternGraph(pattern: StatePattern, expanded = false): PatternGraph {
    const nodeWidth = expanded ? 260 : 190;
    const nodeHeight = expanded ? 92 : 68;
    const controlGap = expanded ? 330 : 238;
    const controlStartX = expanded ? 120 : 86;
    const objectStartY = expanded ? 380 : 292;
    const objectColumnGap = expanded ? 330 : 236;
    const objectRowGap = expanded ? 140 : 104;
    const width = Math.max(
      expanded ? 1320 : 960,
      controlStartX * 2 + Math.max(pattern.sequence.length - 1, 0) * controlGap + nodeWidth,
    );
    const objectColumns = Math.max(1, Math.floor((width - 120) / objectColumnGap));
    const objectRows = Math.max(1, Math.ceil(pattern.object_types.length / objectColumns));
    const height = objectStartY + objectRows * objectRowGap + 54;

    const controlNodes = pattern.sequence.map((label, index) => ({
      id: `control-${index}`,
      lines: wrapGraphLabel(label, expanded ? 31 : 22, expanded ? 5 : 4),
      title: label,
      x: controlStartX + index * controlGap,
      y: 52,
      kind: label.startsWith('CHANGE ') ? ('change' as const) : ('control' as const),
    }));
    const objectNodes = pattern.object_types.map((objectType, index) => ({
      id: `object-${index}`,
      lines: wrapGraphLabel(objectType, expanded ? 31 : 22, expanded ? 5 : 4),
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
  }

  private loadStatePatterns(preserveSelection = false): void {
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
  }

  private loadStateAwareOcdfg(): void {
    if (!this.documentHandle) {
      this.stateAwareOcdfg.set(null);
      return;
    }

    try {
      this.stateAwareOcdfg.set(
        JSON.parse(
          this.documentHandle.stateAwareObjectCentricDirectlyFollowsGraphJson(),
        ) as ProcessGraph,
      );
    } catch (error) {
      this.stateAwareOcdfg.set(null);
      this.errorMessage.set(errorToMessage(error));
    }
  }

  private applyActiveFilter(): void {
    if (!this.documentHandle) {
      return;
    }

    const filter: FilterRequest = {
      event_types: this.selectedEventTypes(),
      object_types: this.selectedObjectTypes(),
    };

    try {
      const nextSummary = JSON.parse(
        this.documentHandle.applyFilter(JSON.stringify(filter)),
      ) as OcelSummary;

      this.summary.set(nextSummary);
      this.originalSummary.set(
        JSON.parse(this.documentHandle.originalSummaryJson()) as OcelSummary,
      );
      this.updateStateMessageAfterFilter(nextSummary);

      if (nextSummary.stateful_events > 0) {
        this.loadStatePatterns(true);
      } else {
        this.patternAnalysis.set(null);
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
  }

  private updateStateMessageAfterFilter(summary: OcelSummary): void {
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
  }
}

function errorToMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === 'string') {
    return error;
  }

  return 'Could not process the OCEL file.';
}

function selectedPattern(patterns: StatePattern[], selectedId: string): StatePattern | null {
  return patterns.find((pattern) => pattern.id === selectedId) ?? patterns[0] ?? null;
}

function emptySummaryValue(): SummaryDisplayValue {
  return {
    current: '0',
    filtered: false,
  };
}

function toggleSelection(values: string[], value: string, checked: boolean): string[] {
  if (checked) {
    return values.includes(value) ? values : [...values, value];
  }

  return values.filter((candidate) => candidate !== value);
}

function filterDescription(prefix: string, values: string[]): string {
  return values.length > 0 ? `${prefix}: ${values.join(', ')}` : `${prefix}: none`;
}

function withLeadingObjectTypeClause(query: string, leadingObjectType: string): string {
  const clause = `FOR LEADING OBJECT TYPE '${escapeSqlString(leadingObjectType)}'`;
  const stateHeader =
    /^\s*STATE\s+([A-Za-z_][A-Za-z0-9_-]*)(?:\s+FOR\s+LEADING\s+OBJECT\s+TYPE\s+(?:"(?:[^"]|"")*"|'(?:[^']|'')*'|[A-Za-z_][A-Za-z0-9_-]*))?\s+AS\s+CASE/im;

  if (stateHeader.test(query)) {
    return query.replace(stateHeader, (_match, attribute: string) => {
      const leadingWhitespace = query.match(/^\s*/)?.[0] ?? '';
      return `${leadingWhitespace}STATE ${attribute} ${clause} AS CASE`;
    });
  }

  return `STATE state ${clause} AS CASE\n  WHEN event.type IS NOT NULL THEN 'State'\nEND`;
}

function escapeSqlString(value: string): string {
  return value.replace(/'/g, "''");
}

function wrapGraphLabel(label: string, maxLineLength: number, maxLines: number): string[] {
  const chunks = label
    .trim()
    .replace(/\s+\[/g, '\n[')
    .split('\n')
    .map((chunk) => chunk.replace(/\s+/g, ' ').trim())
    .filter(Boolean);
  const lines: string[] = [];

  for (const chunk of chunks) {
    let current = '';
    for (const word of chunk.split(' ')) {
      for (const part of splitLongWord(word, maxLineLength)) {
        const candidate = current ? `${current} ${part}` : part;
        if (candidate.length <= maxLineLength) {
          current = candidate;
        } else {
          lines.push(current);
          current = part;
        }
      }
    }
    if (current) {
      lines.push(current);
    }
  }

  if (lines.length <= maxLines) {
    return lines;
  }

  const trimmed = lines.slice(0, maxLines);
  trimmed[maxLines - 1] = `${trimmed[maxLines - 1].slice(0, maxLineLength - 3)}...`;
  return trimmed;
}

function splitLongWord(word: string, maxLineLength: number): string[] {
  const parts: string[] = [];
  for (let index = 0; index < word.length; index += maxLineLength) {
    parts.push(word.slice(index, index + maxLineLength));
  }
  return parts.length > 0 ? parts : [''];
}
