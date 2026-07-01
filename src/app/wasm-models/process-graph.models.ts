export interface ProcessGraphPoint {
  x: number;
  y: number;
}

export interface ProcessGraphNode {
  id: string;
  label: string;
  kind: string;
  shape: 'rect' | 'ellipse';
  color: string;
  object_type?: string;
  count: number;
  x: number;
  y: number;
  width: number;
  height: number;
  lines: string[];
}

export interface ProcessGraphEdge {
  id: string;
  source: string;
  target: string;
  kind: string;
  path: string;
  label: string;
  title: string;
  weight: number;
  object_type: string;
  color: string;
  directed: boolean;
  points: ProcessGraphPoint[];
  label_x: number;
  label_y: number;
  object_types: Array<{ object_type: string; weight: number }>;
}

export interface ProcessGraph {
  title: string;
  subtitle: string;
  width: number;
  height: number;
  nodes: ProcessGraphNode[];
  edges: ProcessGraphEdge[];
}

export interface ProcessGraphSettings {
  object_types: string[];
  min_activity_frequency: number;
  min_path_frequency: number;
}
