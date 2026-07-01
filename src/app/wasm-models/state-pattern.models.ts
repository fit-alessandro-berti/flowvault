export interface StatePatternEdge {
  source: string;
  target: string;
  weight: number;
}

export interface StatePattern {
  id: string;
  family: 'intra' | 'inter';
  label: string;
  leading_object_type: string;
  state?: string;
  from_state?: string;
  to_state?: string;
  support: number;
  mass: number;
  sequence: string[];
  object_types: string[];
  df_edges: StatePatternEdge[];
  eo_edges: StatePatternEdge[];
  oo_edges: StatePatternEdge[];
}

export interface StatePatternAnalysis {
  intra: StatePattern[];
  inter: StatePattern[];
}
