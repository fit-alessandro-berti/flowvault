import { Component } from '@angular/core';
import { AppTopbarComponent } from './components/app-topbar.component';
import { UploadPageComponent } from './components/upload-page.component';
import { FeatureSidebarComponent } from './components/feature-sidebar.component';
import { StatisticsPageComponent } from './components/statistics-page.component';
import { StateDetectionPageComponent } from './components/state-detection-page.component';
import { CausalPageComponent } from './components/causal-page.component';
import { PatternsPageComponent } from './components/patterns-page.component';
import { CorrelationPageComponent } from './components/correlation-page.component';
import { TransitionKpisPageComponent } from './components/transition-kpis-page.component';
import { LifecyclePageComponent } from './components/lifecycle-page.component';
import { TimePerspectivePageComponent } from './components/time-perspective-page.component';
import { GraphPanelsComponent } from './components/graph-panels.component';
import { StateDetectionCellModalComponent } from './components/state-detection-cell-modal.component';
import { FilterModalComponent } from './components/filter-modal.component';
import { StateDialogComponent } from './components/state-dialog.component';
import { LlmConfigModalComponent } from './components/llm-config-modal.component';
import { AppComputedState } from './app-view-model/app-computed-state';
import { appNavigationMethods, type AppNavigationMethods } from './app-view-model/app-navigation.methods';
import { appLlmConfigMethods, type AppLlmConfigMethods } from './app-view-model/app-llm-config.methods';
import { appStateDialogMethods, type AppStateDialogMethods } from './app-view-model/app-state-dialog.methods';
import { appStateLlmMethods, type AppStateLlmMethods } from './app-view-model/app-state-llm.methods';
import { appFileImportMethods, type AppFileImportMethods } from './app-view-model/app-file-import.methods';
import { appFileExportMethods, type AppFileExportMethods } from './app-view-model/app-file-export.methods';
import { appFilterOpenMethods, type AppFilterOpenMethods } from './app-view-model/app-filter-open.methods';
import { appFilterDraftMethods, type AppFilterDraftMethods } from './app-view-model/app-filter-draft.methods';
import { appFilterTimeMethods, type AppFilterTimeMethods } from './app-view-model/app-filter-time.methods';
import { appFilterApplyMethods, type AppFilterApplyMethods } from './app-view-model/app-filter-apply.methods';
import { appStateDetectionMethods, type AppStateDetectionMethods } from './app-view-model/app-state-detection.methods';
import { appCorrelationMethods, type AppCorrelationMethods } from './app-view-model/app-correlation.methods';
import { appTransitionMethods, type AppTransitionMethods } from './app-view-model/app-transition.methods';
import { appLifecycleMethods, type AppLifecycleMethods } from './app-view-model/app-lifecycle.methods';
import { appTimePerspectiveMethods, type AppTimePerspectiveMethods } from './app-view-model/app-time-perspective.methods';
import { appCausalEditorMethods, type AppCausalEditorMethods } from './app-view-model/app-causal-editor.methods';
import { appCausalAnalysisMethods, type AppCausalAnalysisMethods } from './app-view-model/app-causal-analysis.methods';
import { appPatternMethods, type AppPatternMethods } from './app-view-model/app-pattern.methods';
import { appGraphFilterMethods, type AppGraphFilterMethods } from './app-view-model/app-graph-filter.methods';

@Component({
  selector: 'app-root',
  imports: [
    AppTopbarComponent,
    UploadPageComponent,
    FeatureSidebarComponent,
    StatisticsPageComponent,
    StateDetectionPageComponent,
    CausalPageComponent,
    PatternsPageComponent,
    CorrelationPageComponent,
    TransitionKpisPageComponent,
    LifecyclePageComponent,
    TimePerspectivePageComponent,
    GraphPanelsComponent,
    StateDetectionCellModalComponent,
    FilterModalComponent,
    StateDialogComponent,
    LlmConfigModalComponent,
  ],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App extends AppComputedState {}

export interface App extends AppNavigationMethods, AppLlmConfigMethods, AppStateDialogMethods, AppStateLlmMethods, AppFileImportMethods, AppFileExportMethods, AppFilterOpenMethods, AppFilterDraftMethods, AppFilterTimeMethods, AppFilterApplyMethods, AppStateDetectionMethods, AppCorrelationMethods, AppTransitionMethods, AppLifecycleMethods, AppTimePerspectiveMethods, AppCausalEditorMethods, AppCausalAnalysisMethods, AppPatternMethods, AppGraphFilterMethods {}

Object.assign(
  App.prototype,
  appNavigationMethods,
  appLlmConfigMethods,
  appStateDialogMethods,
  appStateLlmMethods,
  appFileImportMethods,
  appFileExportMethods,
  appFilterOpenMethods,
  appFilterDraftMethods,
  appFilterTimeMethods,
  appFilterApplyMethods,
  appStateDetectionMethods,
  appCorrelationMethods,
  appTransitionMethods,
  appLifecycleMethods,
  appTimePerspectiveMethods,
  appCausalEditorMethods,
  appCausalAnalysisMethods,
  appPatternMethods,
  appGraphFilterMethods,
);
