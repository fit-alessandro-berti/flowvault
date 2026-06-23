import { Component, EventEmitter, Input, OnChanges, Output, SimpleChanges } from '@angular/core';
import {
  ProcessGraph,
  ProcessGraphEdge,
  ProcessGraphNode,
  ProcessGraphSettings,
} from './ocel-wasm.service';

@Component({
  selector: 'app-process-graph',
  imports: [],
  templateUrl: './process-graph.component.html',
  styleUrl: './process-graph.component.css',
})
export class ProcessGraphComponent implements OnChanges {
  @Input({ required: true }) graph!: ProcessGraph;
  @Input() objectTypes: string[] = [];
  @Input() settings: ProcessGraphSettings = defaultGraphSettings();
  @Output() applySettings = new EventEmitter<ProcessGraphSettings>();

  protected draftObjectTypes: string[] = [];
  protected draftActivityFrequency = 1;
  protected draftPathFrequency = 1;

  ngOnChanges(changes: SimpleChanges): void {
    if (changes['settings'] || changes['objectTypes']) {
      const availableObjectTypes = new Set(this.objectTypes);
      this.draftObjectTypes = this.settings.object_types.filter((objectType) =>
        availableObjectTypes.has(objectType),
      );
      if (this.draftObjectTypes.length === 0 && this.objectTypes.length > 0) {
        this.draftObjectTypes = [...this.objectTypes];
      }
      this.draftActivityFrequency = Math.max(1, this.settings.min_activity_frequency || 1);
      this.draftPathFrequency = Math.max(1, this.settings.min_path_frequency || 1);
    }
  }

  protected markerId(edge: ProcessGraphEdge): string {
    return `arrow-${edge.id}`;
  }

  protected graphWidth(): number {
    return Math.max(this.graph?.width ?? 0, 980);
  }

  protected maxActivityFrequency(): number {
    return Math.max(
      1,
      ...(this.graph?.nodes ?? [])
        .filter((node) => node.kind !== 'object-start' && node.kind !== 'object-end')
        .map((node) => node.count),
    );
  }

  protected maxPathFrequency(): number {
    return Math.max(1, ...(this.graph?.edges ?? []).map((edge) => edge.weight));
  }

  protected isDraftObjectTypeSelected(objectType: string): boolean {
    return this.draftObjectTypes.includes(objectType);
  }

  protected toggleDraftObjectType(objectType: string, event: Event): void {
    const checked = (event.target as HTMLInputElement).checked;
    this.draftObjectTypes = checked
      ? [...this.draftObjectTypes, objectType]
      : this.draftObjectTypes.filter((selected) => selected !== objectType);
    this.draftObjectTypes = [...new Set(this.draftObjectTypes)];
  }

  protected selectAllObjectTypes(): void {
    this.draftObjectTypes = [...this.objectTypes];
  }

  protected clearObjectTypes(): void {
    this.draftObjectTypes = [];
  }

  protected onActivityFrequencyInput(event: Event): void {
    this.draftActivityFrequency = Number((event.target as HTMLInputElement).value);
  }

  protected onPathFrequencyInput(event: Event): void {
    this.draftPathFrequency = Number((event.target as HTMLInputElement).value);
  }

  protected applyDraftSettings(): void {
    const availableObjectTypes = new Set(this.objectTypes);
    this.applySettings.emit({
      object_types: this.draftObjectTypes.filter((objectType) =>
        availableObjectTypes.has(objectType),
      ),
      min_activity_frequency: clampFrequency(
        this.draftActivityFrequency,
        this.maxActivityFrequency(),
      ),
      min_path_frequency: clampFrequency(this.draftPathFrequency, this.maxPathFrequency()),
    });
  }

  protected edgeStrokeWidth(edge: ProcessGraphEdge): number {
    return Math.min(5.5, 1.25 + Math.log2(edge.weight + 1));
  }

  protected edgeColor(edge: ProcessGraphEdge): string {
    return edge.color || '#31544e';
  }

  protected nodeFill(node: ProcessGraphNode): string {
    if (node.shape === 'ellipse') {
      return node.color || '#31544e';
    }

    if (node.kind === 'state-change') {
      return '#fff0da';
    }

    if (node.kind === 'state-activity') {
      return '#eaf2ff';
    }

    return '#e9f5f1';
  }

  protected nodeFillOpacity(node: ProcessGraphNode): string {
    return node.shape === 'ellipse' ? '0.14' : '1';
  }

  protected nodeStroke(node: ProcessGraphNode): string {
    if (node.shape === 'ellipse') {
      return node.color || '#31544e';
    }

    if (node.kind === 'state-change') {
      return '#b45f1a';
    }

    return node.color || '#9cafaa';
  }

  protected nodeStrokeWidth(node: ProcessGraphNode): number {
    return node.kind === 'state-change' || node.shape === 'ellipse' ? 2 : 1;
  }
}

function defaultGraphSettings(): ProcessGraphSettings {
  return {
    object_types: [],
    min_activity_frequency: 1,
    min_path_frequency: 1,
  };
}

function clampFrequency(value: number, max: number): number {
  if (!Number.isFinite(value)) {
    return 1;
  }

  return Math.min(Math.max(1, Math.round(value)), Math.max(1, max));
}
