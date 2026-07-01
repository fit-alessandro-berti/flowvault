export interface StateTransitionKpiResult {
  object_type: string;
  object_count: number;
  stateful_object_count: number;
  state_count: number;
  states: string[];
  transitions: StateTransitionKpiRow[];
  dwell: StateDwellKpiRow[];
  recovery: StateTransitionKpiRow[];
  stuck: StuckStateRow[];
}

export interface StateTransitionKpiRow {
  from_state: string;
  to_state: string;
  count: number;
  object_count: number;
  min_duration_ms?: number;
  median_duration_ms?: number;
  avg_duration_ms?: number;
  max_duration_ms?: number;
}

export interface StateDwellKpiRow {
  state: string;
  episode_count: number;
  object_count: number;
  total_duration_ms: number;
  min_duration_ms?: number;
  median_duration_ms?: number;
  avg_duration_ms?: number;
  max_duration_ms?: number;
}

export interface StuckStateRow {
  object_id: string;
  state: string;
  entered_time_ms: number;
  last_time_ms: number;
  duration_ms: number;
  event_count: number;
}
