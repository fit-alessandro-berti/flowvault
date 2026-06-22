import { Component, computed, inject, signal } from '@angular/core';
import { exportBaseName, formatHintForFile } from './ocel-file';
import { presetsForFile, StateQueryPreset } from './state-query-presets';
import {
  OcelDocumentHandle,
  StatePattern,
  StatePatternAnalysis,
  StatePatternEdge,
  OcelSummary,
  OcelWasmService,
  StateQueryResult,
} from './ocel-wasm.service';

interface SummaryCard {
  label: string;
  value: number;
}

type PatternVisualization = 'text' | 'graph';

interface PatternGraphNode {
  id: string;
  label: string;
  title: string;
  x: number;
  y: number;
  kind: 'control' | 'object';
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
  nodes: PatternGraphNode[];
  edges: PatternGraphEdge[];
}

@Component({
  selector: 'app-root',
  imports: [],
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
  protected readonly stateQueryDraft = signal('');
  protected readonly summary = signal<OcelSummary | null>(null);
  protected readonly patternAnalysis = signal<StatePatternAnalysis | null>(null);
  protected readonly selectedIntraPatternId = signal('');
  protected readonly selectedInterPatternId = signal('');
  protected readonly intraVisualization = signal<PatternVisualization>('text');
  protected readonly interVisualization = signal<PatternVisualization>('text');
  protected readonly hasDocument = computed(() => this.summary() !== null);
  protected readonly stateQueryPresets = computed(() => presetsForFile(this.fileName()));
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
      { label: 'Events', value: summary?.events ?? 0 },
      { label: 'Objects', value: summary?.objects ?? 0 },
      { label: 'E2O', value: summary?.e2o_relationships ?? 0 },
      { label: 'O2O', value: summary?.o2o_relationships ?? 0 },
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
      this.selectedPresetId.set(selectedPreset.id);
      this.stateQueryDraft.set(selectedPreset.query);
    }

    this.errorMessage.set('');
    this.isStateDialogOpen.set(true);
  }

  closeStateDialog(): void {
    this.isStateDialogOpen.set(false);
  }

  selectStatePreset(preset: StateQueryPreset): void {
    this.selectedPresetId.set(preset.id);
    this.stateQueryDraft.set(preset.query);
  }

  onStateQueryDraftChange(event: Event): void {
    this.stateQueryDraft.set((event.target as HTMLTextAreaElement).value);
  }

  applyStateQuery(): void {
    if (!this.documentHandle) {
      return;
    }

    this.errorMessage.set('');
    this.stateMessage.set('');

    try {
      const result = JSON.parse(
        this.documentHandle.applyStateQuery(this.stateQueryDraft()),
      ) as StateQueryResult;
      this.summary.set(JSON.parse(this.documentHandle.summaryJson()) as OcelSummary);
      this.loadStatePatterns();
      this.stateMessage.set(
        `Added ${result.attribute} to ${result.assigned_events.toLocaleString()} of ${result.total_events.toLocaleString()} events.`,
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
      this.stateMessage.set('');
      this.patternAnalysis.set(null);
      this.selectedIntraPatternId.set('');
      this.selectedInterPatternId.set('');
      this.isStateDialogOpen.set(false);
      this.initializeStatePresetForFile(file.name);
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
      this.summary.set(null);
      this.fileName.set(file.name);
      this.documentHandle?.free();
      this.documentHandle = undefined;
      this.stateMessage.set('');
      this.patternAnalysis.set(null);
      this.selectedIntraPatternId.set('');
      this.selectedInterPatternId.set('');
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
    this.selectedPresetId.set(preset.id);
    this.stateQueryDraft.set(preset.query);
  }

  protected selectIntraPattern(event: Event): void {
    this.selectedIntraPatternId.set((event.target as HTMLSelectElement).value);
  }

  protected selectInterPattern(event: Event): void {
    this.selectedInterPatternId.set((event.target as HTMLSelectElement).value);
  }

  protected setIntraVisualization(visualization: PatternVisualization): void {
    this.intraVisualization.set(visualization);
  }

  protected setInterVisualization(visualization: PatternVisualization): void {
    this.interVisualization.set(visualization);
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

  protected patternGraph(pattern: StatePattern): PatternGraph {
    const controlGap = 148;
    const controlWidth = 132;
    const controlStartX = 72;
    const objectStartY = 218;
    const width = Math.max(
      760,
      controlStartX * 2 + Math.max(pattern.sequence.length - 1, 0) * controlGap + controlWidth,
    );
    const objectColumns = Math.max(1, Math.floor((width - 96) / 174));
    const objectRows = Math.max(1, Math.ceil(pattern.object_types.length / objectColumns));
    const height = objectStartY + objectRows * 76 + 42;

    const controlNodes = pattern.sequence.map((label, index) => ({
      id: `control-${index}`,
      label: compactGraphLabel(label),
      title: label,
      x: controlStartX + index * controlGap,
      y: 52,
      kind: 'control' as const,
    }));
    const objectNodes = pattern.object_types.map((objectType, index) => ({
      id: `object-${index}`,
      label: compactGraphLabel(objectType),
      title: objectType,
      x: 72 + (index % objectColumns) * 174,
      y: objectStartY + Math.floor(index / objectColumns) * 76,
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
        x1: source.x + controlWidth,
        y1: source.y + 23,
        x2: target.x,
        y2: target.y + 23,
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
        x1: source.x + controlWidth / 2,
        y1: source.y + 46,
        x2: target.x + controlWidth / 2,
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
        x1: source.x + controlWidth,
        y1: source.y + 23,
        x2: target.x,
        y2: target.y + 23,
        label: edge.weight.toLocaleString(),
        kind: 'oo',
      });
    }

    return { width, height, nodes, edges };
  }

  private loadStatePatterns(): void {
    if (!this.documentHandle) {
      this.patternAnalysis.set(null);
      return;
    }

    const analysis = JSON.parse(this.documentHandle.statePatternsJson()) as StatePatternAnalysis;
    this.patternAnalysis.set(analysis);
    this.selectedIntraPatternId.set(analysis.intra[0]?.id ?? '');
    this.selectedInterPatternId.set(analysis.inter[0]?.id ?? '');
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

function compactGraphLabel(label: string): string {
  if (label.length <= 28) {
    return label;
  }

  return `${label.slice(0, 25)}...`;
}
