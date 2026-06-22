import { Component, computed, inject, signal } from '@angular/core';
import { exportBaseName, formatHintForFile } from './ocel-file';
import {
  OcelDocumentHandle,
  OcelSummary,
  OcelWasmService,
  StateQueryResult,
} from './ocel-wasm.service';

interface SummaryCard {
  label: string;
  value: number;
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
  protected readonly stateQuery = signal(DEFAULT_STATE_QUERY);
  protected readonly summary = signal<OcelSummary | null>(null);
  protected readonly hasDocument = computed(() => this.summary() !== null);
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

  onStateQueryChange(event: Event): void {
    this.stateQuery.set((event.target as HTMLTextAreaElement).value);
  }

  applyStateQuery(): void {
    if (!this.documentHandle) {
      return;
    }

    this.errorMessage.set('');
    this.stateMessage.set('');

    try {
      const result = JSON.parse(
        this.documentHandle.applyStateQuery(this.stateQuery()),
      ) as StateQueryResult;
      this.summary.set(JSON.parse(this.documentHandle.summaryJson()) as OcelSummary);
      this.stateMessage.set(
        `Added ${result.attribute} to ${result.assigned_events.toLocaleString()} of ${result.total_events.toLocaleString()} events.`,
      );
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
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
      this.summary.set(null);
      this.fileName.set(file.name);
      this.documentHandle?.free();
      this.documentHandle = undefined;
      this.stateMessage.set('');
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
}

const DEFAULT_STATE_QUERY = `STATE state AS CASE
  WHEN object.status IS NOT NULL THEN object.status
  WHEN object.state IS NOT NULL THEN object.state
  WHEN object.is_blocked = 'Yes' THEN 'Blocked'
  WHEN event.type LIKE '%cancel%' THEN 'Exception'
  ELSE 'Normal'
END`;

function errorToMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === 'string') {
    return error;
  }

  return 'Could not process the OCEL file.';
}
