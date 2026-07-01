import type { StatePattern } from '../ocel-wasm.service';

export type PatternTab = 'intra' | 'inter';

export type PatternVisualization = 'text' | 'graph';

export interface PatternGraphNode {
  id: string;
  lines: string[];
  title: string;
  x: number;
  y: number;
  kind: 'control' | 'change' | 'object';
}

export interface PatternGraphEdge {
  id: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  label: string;
  kind: 'df' | 'eo' | 'oo';
}

export interface PatternGraph {
  width: number;
  height: number;
  nodeWidth: number;
  nodeHeight: number;
  nodes: PatternGraphNode[];
  edges: PatternGraphEdge[];
}

export interface PatternExplorerRow {
  pattern: StatePattern;
  graph: PatternGraph;
}

export interface StaticSampleLog {
  label: string;
  detail: string;
  fileName: string;
  path: string;
}
