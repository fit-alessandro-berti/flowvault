export interface StateCorrelationResult {
  object_type: string;
  object_count: number;
  stateful_object_count: number;
  state_count: number;
  feature_count: number;
  state_distribution: StateCorrelationStateCount[];
  rows: StateCorrelationRow[];
}

export interface StateCorrelationStateCount {
  state: string;
  count: number;
}

export interface StateCorrelationRow {
  feature: string;
  state: string;
  correlation: number;
  strength: number;
  sample_count: number;
  state_count: number;
  mean_in_state: number;
  mean_outside_state: number;
}
