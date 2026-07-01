import type { StateCorrelationResult, TimePerspectiveResult } from '../ocel-wasm.service';

export const stateCorrelationAnalysis: StateCorrelationResult = {
  object_type: 'Order',
  object_count: 10,
  stateful_object_count: 8,
  state_count: 2,
  feature_count: 2,
  state_distribution: [
    { state: 'Open', count: 5 },
    { state: 'Closed', count: 3 },
  ],
  rows: [
    {
      feature: 'activity.Create Order',
      state: 'Open',
      correlation: 0.82,
      strength: 0.82,
      sample_count: 8,
      state_count: 5,
      mean_in_state: 1.4,
      mean_outside_state: 0.3,
    },
    {
      feature: 'activity.Close Order',
      state: 'Closed',
      correlation: -0.64,
      strength: 0.64,
      sample_count: 8,
      state_count: 3,
      mean_in_state: 0.2,
      mean_outside_state: 1.1,
    },
  ],
};

export const transitionKpisAnalysis = {
  object_type: 'Order',
  object_count: 10,
  stateful_object_count: 8,
  state_count: 2,
  states: ['Open', 'Closed'],
  transitions: [
    {
      from_state: 'Open',
      to_state: 'Closed',
      count: 6,
      object_count: 5,
      min_duration_ms: 60_000,
      median_duration_ms: 120_000,
      avg_duration_ms: 150_000,
      max_duration_ms: 240_000,
    },
  ],
  dwell: [
    {
      state: 'Open',
      episode_count: 7,
      object_count: 6,
      total_duration_ms: 600_000,
      min_duration_ms: 60_000,
      median_duration_ms: 120_000,
      avg_duration_ms: 140_000,
      max_duration_ms: 300_000,
    },
  ],
  recovery: [
    {
      from_state: 'Open',
      to_state: 'Closed',
      count: 6,
      object_count: 5,
      min_duration_ms: 60_000,
      median_duration_ms: 120_000,
      avg_duration_ms: 150_000,
      max_duration_ms: 240_000,
    },
  ],
  stuck: [
    {
      object_id: 'O1',
      state: 'Open',
      entered_time_ms: 0,
      last_time_ms: 240_000,
      duration_ms: 240_000,
      event_count: 3,
    },
  ],
};

export const timePerspectiveAnalysis: TimePerspectiveResult = {
  object_type: 'Order',
  event_min_ms: 0,
  event_max_ms: 240_000,
  states: ['Open', 'Closed'],
  buckets: [
    {
      start_ms: 0,
      end_ms: 120_000,
      total: 2,
      percentages: [
        { state: 'Open', percentage: 100, count: 2 },
        { state: 'Closed', percentage: 0, count: 0 },
      ],
    },
    {
      start_ms: 120_000,
      end_ms: 240_000,
      total: 2,
      percentages: [
        { state: 'Open', percentage: 25, count: 1 },
        { state: 'Closed', percentage: 75, count: 3 },
      ],
    },
  ],
  performance: {
    object_type: 'Order',
    from_state: 'Open',
    to_state: 'Closed',
    roundtrip: false,
    sample_count: 1,
    min_duration_ms: 120_000,
    median_duration_ms: 120_000,
    avg_duration_ms: 120_000,
    max_duration_ms: 120_000,
    samples: [
      {
        object_id: 'O1',
        start_time_ms: 0,
        middle_time_ms: 120_000,
        duration_ms: 120_000,
      },
    ],
  },
};

export const objectSearchResult = {
  objects: [{ object_id: 'O1', object_type: 'Order', event_count: 3 }],
};

export const lifecycleDetail = {
  object_id: 'O1',
  object_type: 'Order',
  event_count: 3,
  event_min_ms: 0,
  event_max_ms: 240_000,
  state_bands: [
    {
      state: 'Open',
      start_time_ms: 0,
      end_time_ms: 120_000,
      event_count: 2,
      start_event_id: 'e1',
      end_event_id: 'e2',
    },
    {
      state: 'Closed',
      start_time_ms: 240_000,
      end_time_ms: 240_000,
      event_count: 1,
      start_event_id: 'e3',
      end_event_id: 'e3',
    },
  ],
  stock_points: [
    { name: 'Stock After', time_ms: 0, value: 10, event_id: 'e1' },
    { name: 'Stock After', time_ms: 240_000, value: 20, event_id: 'e3' },
  ],
  related_objects: [
    { object_id: 'I1', object_type: 'Item', qualifier: 'contains', event_count: 2 },
  ],
  events: [
    {
      event_id: 'e1',
      event_type: 'Create Order',
      time_ms: 0,
      state: 'Open',
      attributes: [{ name: 'Stock After', value: 10 }],
      related_objects: [{ object_id: 'I1', object_type: 'Item', qualifier: 'contains' }],
    },
    {
      event_id: 'e2',
      event_type: 'Pick Item',
      time_ms: 120_000,
      state: 'Open',
      attributes: [],
      related_objects: [{ object_id: 'I1', object_type: 'Item', qualifier: 'contains' }],
    },
    {
      event_id: 'e3',
      event_type: 'Close Order',
      time_ms: 240_000,
      state: 'Closed',
      attributes: [{ name: 'Stock After', value: 20 }],
      related_objects: [],
    },
  ],
};
