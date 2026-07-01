export interface ObjectSearchResult {
  objects: ObjectSearchHit[];
}

export interface ObjectSearchHit {
  object_id: string;
  object_type: string;
  event_count: number;
}

export interface ObjectLifecycleDetail {
  object_id: string;
  object_type: string;
  event_count: number;
  event_min_ms?: number;
  event_max_ms?: number;
  events: LifecycleEventDetail[];
  state_bands: LifecycleStateBand[];
  stock_points: LifecycleStockPoint[];
  related_objects: LifecycleRelatedObjectSummary[];
}

export interface LifecycleEventDetail {
  event_id: string;
  event_type: string;
  time_ms: number;
  state?: string;
  attributes: LifecycleAttribute[];
  related_objects: LifecycleRelatedObject[];
}

export interface LifecycleAttribute {
  name: string;
  value: string | number | boolean;
}

export interface LifecycleRelatedObject {
  object_id: string;
  object_type: string;
  qualifier: string;
}

export interface LifecycleRelatedObjectSummary {
  object_id: string;
  object_type: string;
  qualifier: string;
  event_count: number;
}

export interface LifecycleStateBand {
  state: string;
  start_time_ms: number;
  end_time_ms: number;
  event_count: number;
  start_event_id: string;
  end_event_id: string;
}

export interface LifecycleStockPoint {
  name: string;
  time_ms: number;
  value: number;
  event_id: string;
}
