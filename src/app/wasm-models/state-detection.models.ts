import type { ProcessGraph } from './process-graph.models';

export interface StateDetectionResult {
  object_type: string;
  window_size: number;
  som_width: number;
  som_height: number;
  object_count: number;
  feature_count: number;
  window_count: number;
  color_attribute: string;
  color_attributes: StateDetectionColorOption[];
  feature_columns: string[];
  table_preview: StateDetectionPreviewRow[];
  pca: StateDetectionPca;
  som: StateDetectionSom;
  windows: StateDetectionWindow[];
}

export interface StateDetectionColorOption {
  id: string;
  label: string;
  kind: 'count' | 'numeric' | 'categorical';
}

export interface StateDetectionPreviewRow {
  object_id: string;
  values: number[];
}

export interface StateDetectionPca {
  pc1_variance: number;
  pc2_variance: number;
  pc1_explained_ratio: number;
  pc2_explained_ratio: number;
}

export interface StateDetectionSom {
  cells: StateDetectionSomCell[];
  transitions: StateDetectionSomTransition[];
}

export interface StateDetectionSomCell {
  x: number;
  y: number;
  label: string;
  count: number;
  color_value: number;
  color_label: string;
  color_kind: string;
  avg_pc1: number;
  avg_pc2: number;
  dominant_activity?: string;
}

export interface StateDetectionSomTransition {
  source_x: number;
  source_y: number;
  target_x: number;
  target_y: number;
  count: number;
  distance: number;
  nearby: boolean;
}

export interface StateDetectionWindow {
  object_id: string;
  start_event: string;
  end_event: string;
  pc1: number;
  pc2: number;
  cell_x: number;
  cell_y: number;
}

export interface StateDetectionCellDetail {
  cell: StateDetectionSomCell;
  dfg: ProcessGraph;
  entering_dfg: ProcessGraph;
  exiting_dfg: ProcessGraph;
  entering_window_count: number;
  exiting_window_count: number;
  entering_windows: StateDetectionBoundaryWindow[];
  exiting_windows: StateDetectionBoundaryWindow[];
}

export interface StateDetectionBoundaryWindow {
  object_id: string;
  start_event: string;
  end_event: string;
  source_cell: string;
  target_cell: string;
  pc1: number;
  pc2: number;
  activities: string[];
}
