import { presetsForFile, type StateQueryPreset } from '../state-query-presets';
import type {
  StateConfiguration,
  StateComposition,
  StateDefinition,
  StateQueryResult,
  OcelFilterOptions,
  OcelSummary,
} from '../ocel-wasm.service';
import {
  LLM_STATE_PRESET_ID,
  SAVED_STATE_PRESET_ID,
  defaultStateQuery,
} from '../helpers/state-expression.helpers';
import { errorToMessage, withLeadingObjectTypeClause } from '../helpers/common.helpers';

export const appStateDialogMethods = {
  openStateDialog(this: any): void {
    if (!this.documentHandle) {
      return;
    }

    this.refreshStateConfiguration();
    const presets = this.stateQueryPresets();
    const selectedPreset =
      presets.find((preset: StateQueryPreset) => preset.id === this.selectedPresetId()) ??
      presets[0];

    if (!this.stateQueryDraft().trim() && selectedPreset) {
      this.selectStatePreset(selectedPreset);
    } else {
      this.ensureLeadingObjectTypeSelection();
    }

    this.errorMessage.set('');
    this.isExportMenuOpen.set(false);
    this.isFilterMenuOpen.set(false);
    this.isFilterChainOpen.set(false);
    this.graphFilterMenu.set(null);
    this.isStateDialogOpen.set(true);
  },

  closeStateDialog(this: any): void {
    this.isStateDialogOpen.set(false);
  },

  selectStatePreset(this: any, preset: StateQueryPreset): void {
    const leadingObjectType = this.validLeadingObjectType(preset.leadingObjectType);
    this.selectedPresetId.set(preset.id);
    this.selectedLeadingObjectType.set(leadingObjectType);
    this.stateDialogMode.set('attribute');
    this.setStateQueryTemplate(preset.query);
  },

  selectPersistedStateExpression(this: any): void {
    const expression = this.persistedStateExpression();
    if (!expression) {
      return;
    }

    this.ensureLeadingObjectTypeSelection();
    this.stateDialogMode.set('attribute');
    this.selectedPresetId.set(SAVED_STATE_PRESET_ID);
    this.setStateQueryTemplate(expression);
  },

  setStateQueryTemplate(this: any, expression: string): void {
    const attribute = this.stateAttributeName() || 'state';
    this.stateQueryDraft.set(
      withLeadingObjectTypeClause(expression, this.selectedLeadingObjectType()).replace(
        /^(\s*STATE\s+)[A-Za-z_][A-Za-z0-9_-]*/im,
        (_: string, prefix: string) => `${prefix}${attribute}`,
      ),
    );
  },

  selectLlmStateExpression(this: any): void {
    this.ensureLeadingObjectTypeSelection();
    this.stateDialogMode.set('attribute');
    this.selectedPresetId.set(LLM_STATE_PRESET_ID);
    if (!this.stateQueryDraft().trim()) {
      this.stateQueryDraft.set(defaultStateQuery(this.selectedLeadingObjectType()));
    }
  },

  onStateQueryDraftChange(this: any, event: Event): void {
    this.stateQueryDraft.set((event.target as HTMLTextAreaElement).value);
  },

  onLlmStatePromptChange(this: any, event: Event): void {
    this.llmStatePrompt.set((event.target as HTMLTextAreaElement).value);
  },

  onLeadingObjectTypeChange(this: any, event: Event): void {
    const leadingObjectType = (event.target as HTMLSelectElement).value;
    this.selectedLeadingObjectType.set(leadingObjectType);
    this.stateQueryDraft.set(
      withLeadingObjectTypeClause(this.stateQueryDraft(), leadingObjectType),
    );
  },

  applyStateQuery(this: any): void {
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
      this.persistStateExpression(query);
      this.refreshStateConfiguration();
      this.refreshStateAnalysis();
      this.activeFeature.set(this.summary()?.stateful_events > 0 ? 'patterns' : 'statistics');
      this.stateMessage.set(
        `Added ${result.attribute} for ${result.leading_object_type} to ${result.assigned_events.toLocaleString()} of ${result.total_events.toLocaleString()} events.`,
      );
      this.isStateDialogOpen.set(false);
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  },

  initializeStatePresetForFile(this: any, fileName: string): void {
    this.stateDefinitions.set([]);
    this.stateDialogMode.set('attribute');
    this.stateQueryDraft.set('');
    this.stateCompositionDraft.set({
      leading_object_type: this.validLeadingObjectType(''),
      state_expression: 'event.state',
      activity_expression: "CONCAT(event.type, ' [', event.combined_state, ']')",
    });
    const preset = presetsForFile(fileName, this.filterOptions().object_types)[0];
    if (preset) {
      this.selectStatePreset(preset);
      return;
    }

    this.selectedPresetId.set('');
    this.ensureLeadingObjectTypeSelection();
    this.stateQueryDraft.set(defaultStateQuery(this.selectedLeadingObjectType()));
  },

  stateAttributeName(this: any): string {
    return (
      this.stateQueryDraft().match(/^\s*STATE\s+([A-Za-z_][A-Za-z0-9_-]*)\s+FOR\b/i)?.[1] ?? ''
    );
  },

  onStateAttributeNameChange(this: any, event: Event): void {
    const name = (event.target as HTMLInputElement).value;
    this.stateQueryDraft.set(
      this.stateQueryDraft().replace(
        /^(\s*STATE\s+).*?(\s+FOR\b)/i,
        (_: string, prefix: string, suffix: string) => `${prefix}${name}${suffix}`,
      ),
    );
  },

  addStateAttribute(this: any): void {
    this.ensureLeadingObjectTypeSelection();
    const names = new Set(this.stateDefinitions().map((item: StateDefinition) => item.attribute));
    let index = 1;
    while (names.has(`state${index}`)) index++;
    this.stateQueryDraft.set(
      defaultStateQuery(this.selectedLeadingObjectType()).replace(
        'STATE state ',
        `STATE state${index} `,
      ),
    );
    this.stateDialogMode.set('attribute');
    this.selectedPresetId.set('');
  },

  selectStateDefinition(this: any, definition: StateDefinition): void {
    this.stateDialogMode.set('attribute');
    this.selectedPresetId.set('');
    this.selectedLeadingObjectType.set(definition.leading_object_type);
    this.stateQueryDraft.set(definition.query);
  },

  refreshStateConfiguration(this: any): void {
    if (!this.documentHandle?.stateConfigurationJson) return;
    const configuration = JSON.parse(
      this.documentHandle.stateConfigurationJson(),
    ) as StateConfiguration;
    this.stateDefinitions.set(configuration.definitions);
    if (configuration.composition) this.stateCompositionDraft.set(configuration.composition);
  },

  openStateComposition(this: any): void {
    this.refreshStateConfiguration();
    this.stateDialogMode.set('composition');
  },

  onStateCompositionChange(this: any, key: keyof StateComposition, event: Event): void {
    this.stateCompositionDraft.update((draft: StateComposition) => ({
      ...draft,
      [key]: (event.target as HTMLInputElement).value,
    }));
  },

  useAllStateAttributes(this: any, withActivity: boolean): void {
    const fields = this.stateDefinitions().map(
      (definition: StateDefinition) => `event."${definition.attribute.replace(/"/g, '""')}"`,
    );
    if (!fields.length) return;
    const expression = fields.length === 1 ? fields[0] : `CONCAT(${fields.join(", ' / ', ")})`;
    this.stateCompositionDraft.update((draft: StateComposition) => ({
      ...draft,
      state_expression: expression,
      activity_expression: withActivity
        ? "CONCAT(event.type, ' [', event.combined_state, ']')"
        : 'event.combined_state',
    }));
  },

  applyStateComposition(this: any): void {
    if (!this.documentHandle) return;
    try {
      const result = JSON.parse(
        this.documentHandle.applyStateComposition(JSON.stringify(this.stateCompositionDraft())),
      ) as StateQueryResult;
      this.refreshStateConfiguration();
      this.refreshStateAnalysis();
      this.activeFeature.set(
        this.summary()?.stateful_events > 0 ? 'stateAwareOcdfg' : 'statistics',
      );
      this.stateMessage.set(
        `Updated activity composition for ${result.assigned_events.toLocaleString()} of ${result.total_events.toLocaleString()} events.`,
      );
      this.errorMessage.set('');
      this.isStateDialogOpen.set(false);
    } catch (error) {
      this.errorMessage.set(errorToMessage(error));
    }
  },

  refreshStateAnalysis(this: any): void {
    this.filterOptions.set(
      JSON.parse(this.documentHandle.filterOptionsJson()) as OcelFilterOptions,
    );
    this.summary.set(JSON.parse(this.documentHandle.summaryJson()) as OcelSummary);
    this.originalSummary.set(JSON.parse(this.documentHandle.originalSummaryJson()) as OcelSummary);
    this.stateCorrelation.set(null);
    this.timePerspective.set(null);
    this.stateTransitionKpis.set(null);
    this.lifecycleDetail.set(null);
    if (this.summary()?.stateful_events > 0) {
      this.loadStatePatterns();
    } else {
      this.patternAnalysis.set(null);
      this.stateAwareOcdfg.set(null);
      this.selectedIntraPatternId.set('');
      this.selectedInterPatternId.set('');
      this.fullScreenPattern.set(null);
    }
  },

  ensureLeadingObjectTypeSelection(this: any): void {
    this.selectedLeadingObjectType.set(
      this.validLeadingObjectType(this.selectedLeadingObjectType()),
    );
  },

  validLeadingObjectType(this: any, candidate: string): string {
    const options = this.leadingObjectTypeOptions();
    if (candidate && options.includes(candidate)) {
      return candidate;
    }
    return options[0] ?? this.filterOptions().object_types[0] ?? candidate;
  },
};

export type AppStateDialogMethods = typeof appStateDialogMethods;
