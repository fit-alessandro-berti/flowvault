import type { CausalFitEdge } from '../ocel-wasm.service';

export type CausalNodeRole = 'observable' | 'latent' | 'outcome';

export type CausalOperation = 'identity' | 'log10' | 'log_e' | 'sqrt';

export interface CausalModelNode {
  id: string;
  label: string;
  role: CausalNodeRole;
  feature?: string;
  operation: CausalOperation;
}

export interface CausalModelEdge {
  source: string;
  target: string;
}

export interface CausalFitGraphNode {
  id: string;
  label: string;
  role: CausalNodeRole;
  x: number;
  y: number;
  width: number;
  height: number;
  lines: string[];
}

export interface CausalFitGraphEdge {
  id: string;
  source: CausalFitGraphNode;
  target: CausalFitGraphNode;
  edge: CausalFitEdge;
  path: string;
  labelX: number;
  labelY: number;
}

export interface CausalFitGraph {
  width: number;
  height: number;
  nodes: CausalFitGraphNode[];
  edges: CausalFitGraphEdge[];
}
