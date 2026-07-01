export interface OcelFilterOptions {
  event_types: string[];
  object_types: string[];
  text_attributes: TextAttributeOption[];
  time_min_ms?: number;
  time_max_ms?: number;
  time_buckets: FilterTimeBucket[];
}

export interface FilterTimeBucket {
  start_ms: number;
  end_ms: number;
  count: number;
}

export interface TextAttributeOption {
  scope: 'event' | 'object';
  name: string;
  values: string[];
}
