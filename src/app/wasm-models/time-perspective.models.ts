export interface TimePerspectiveResult {
  object_type: string;
  event_min_ms: number;
  event_max_ms: number;
  states: string[];
  buckets: TimeFrequencyBucket[];
  performance: TimePerformanceSpectrum;
}

export interface TimeFrequencyBucket {
  start_ms: number;
  end_ms: number;
  total: number;
  percentages: TimeStatePercentage[];
}

export interface TimeStatePercentage {
  state: string;
  percentage: number;
  count: number;
}

export interface TimePerformanceSpectrum {
  object_type: string;
  from_state: string;
  to_state: string;
  roundtrip: boolean;
  sample_count: number;
  min_duration_ms?: number;
  median_duration_ms?: number;
  avg_duration_ms?: number;
  max_duration_ms?: number;
  samples: TimePerformanceSample[];
}

export interface TimePerformanceSample {
  object_id: string;
  start_time_ms: number;
  middle_time_ms: number;
  end_time_ms?: number;
  duration_ms: number;
}
