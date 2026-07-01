import { computed } from '@angular/core';
import { causalFitGraph } from '../helpers/causal-fit-graph.helpers';
import { fromDateTimeLocalInput, timeFilterCurve } from '../helpers/time-range.helpers';
import { lifecycleTimelineView, transitionMatrixView } from '../helpers/transition-lifecycle.helpers';
import { performanceSpectrumChart, timeFrequencyChart } from '../helpers/time-chart.helpers';
import { AppFilterComputedState } from './app-computed-filters';

export class AppComputedState extends AppFilterComputedState {
  protected readonly causalObservableNodes = computed(() =>
    this.causalNodes().filter((node) => node.role === 'observable'),
  );
  protected readonly causalLatentNodes = computed(() =>
    this.causalNodes().filter((node) => node.role === 'latent'),
  );
  protected readonly causalOutcomeNodes = computed(() =>
    this.causalNodes().filter((node) => node.role === 'outcome'),
  );
  protected readonly canFitCausalModel = computed(
    () =>
      this.causalObservableNodes().length > 0 &&
      this.causalLatentNodes().length > 0 &&
      this.causalOutcomeNodes().length > 0 &&
      this.causalEdges().length > 0,
  );
  protected readonly causalFitGraph = computed(() => {
    const fit = this.causalFit();
    return fit ? causalFitGraph(fit) : null;
  });
  protected readonly timeFrequencyChart = computed(() => {
    const analysis = this.timePerspective();
    return analysis ? timeFrequencyChart(analysis.buckets, analysis.states) : null;
  });
  protected readonly timeFilterCurve = computed(() => {
    const options = this.filterOptions();
    return timeFilterCurve(
      options.time_buckets,
      fromDateTimeLocalInput(this.draftTimeStart()),
      fromDateTimeLocalInput(this.draftTimeEnd()),
    );
  });
  protected readonly performanceSpectrumChart = computed(() => {
    const analysis = this.timePerspective();
    return analysis
      ? performanceSpectrumChart(analysis.performance, analysis.event_min_ms, analysis.event_max_ms)
      : null;
  });
  protected readonly transitionMatrixView = computed(() => {
    const analysis = this.stateTransitionKpis();
    return analysis ? transitionMatrixView(analysis) : null;
  });
  protected readonly lifecycleTimelineView = computed(() => {
    const detail = this.lifecycleDetail();
    return detail ? lifecycleTimelineView(detail) : null;
  });
  protected readonly timePerspectiveToStateOptions = computed(() => {
    const analysis = this.timePerspective();
    return analysis
      ? analysis.states.filter((state) => state !== this.timePerspectiveFromState())
      : [];
  });
}
