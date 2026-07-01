import type { StateDetectionPreviewRow } from './state-detection.models';

export interface CausalFeatureTableResult {
  object_type: string;
  object_count: number;
  feature_count: number;
  feature_columns: string[];
  table_preview: StateDetectionPreviewRow[];
}

export interface CausalFitResult {
  object_type: string;
  sample_count: number;
  nodes: CausalFitNode[];
  edges: CausalFitEdge[];
}

export interface CausalFitNode {
  id: string;
  label: string;
  role: 'observable' | 'latent' | 'outcome';
  feature?: string;
  operation: string;
  mean: number;
  std_dev: number;
}

export interface CausalFitEdge {
  source: string;
  target: string;
  correlation: number;
  intensity: number;
  p_value: number;
  sample_count: number;
}
