import type { ProcessGraph, StatePatternAnalysis } from '../ocel-wasm.service';

export const importedSummary = {
  source_format: 'json' as const,
  event_types: 1,
  object_types: 2,
  events: 2,
  objects: 2,
  e2o_relationships: 3,
  o2o_relationships: 1,
  interned_strings: 8,
  objects_with_lifecycle: 2,
  stateful_events: 0,
};

export const statefulSummary = {
  ...importedSummary,
  stateful_events: 2,
};

export const patternAnalysis: StatePatternAnalysis = {
  intra: [
    {
      id: 'intra-1',
      family: 'intra',
      label: 'Open on Order',
      leading_object_type: 'Order',
      state: 'Open',
      support: 5,
      mass: 10,
      sequence: ['START Open', 'Create Order [Open]', 'END Open'],
      object_types: ['Order', 'Item'],
      df_edges: [
        { source: 'START Open', target: 'Create Order [Open]', weight: 5 },
        { source: 'Create Order [Open]', target: 'END Open', weight: 5 },
      ],
      eo_edges: [{ source: 'Create Order [Open]', target: 'Item', weight: 5 }],
      oo_edges: [{ source: 'Order', target: 'Item', weight: 5 }],
    },
  ],
  inter: [
    {
      id: 'inter-1',
      family: 'inter',
      label: 'Open -> Closed on Order',
      leading_object_type: 'Order',
      from_state: 'Open',
      to_state: 'Closed',
      support: 3,
      mass: 9,
      sequence: [
        'START Open',
        'Create Order [Open]',
        'CHANGE Open -> Closed',
        'Close Order [Closed]',
        'END Closed',
      ],
      object_types: ['Order', 'Item'],
      df_edges: [
        { source: 'START Open', target: 'Create Order [Open]', weight: 3 },
        { source: 'Create Order [Open]', target: 'CHANGE Open -> Closed', weight: 3 },
        { source: 'CHANGE Open -> Closed', target: 'Close Order [Closed]', weight: 3 },
        { source: 'Close Order [Closed]', target: 'END Closed', weight: 3 },
      ],
      eo_edges: [{ source: 'Close Order [Closed]', target: 'Item', weight: 3 }],
      oo_edges: [{ source: 'Order', target: 'Item', weight: 3 }],
    },
  ],
};

export const processGraph: ProcessGraph = {
  title: 'State-Aware Object-Centric Directly-Follows Graph',
  subtitle: 'State-enriched lifecycles collated across object types',
  width: 520,
  height: 220,
  nodes: [
    {
      id: 'n0',
      label: 'START\nOrder',
      kind: 'object-start',
      shape: 'ellipse',
      color: 'hsl(214 68% 38%)',
      object_type: 'Order',
      count: 1,
      x: 20,
      y: 60,
      width: 140,
      height: 72,
      lines: ['START', 'Order'],
    },
    {
      id: 'n1',
      label: 'Create Order [Open]',
      kind: 'state-activity',
      shape: 'rect',
      color: '#42635c',
      count: 2,
      x: 40,
      y: 60,
      width: 180,
      height: 68,
      lines: ['Create Order', '[Open]'],
    },
    {
      id: 'n2',
      label: 'CHANGE Open -> Closed',
      kind: 'state-change',
      shape: 'rect',
      color: '#42635c',
      count: 1,
      x: 300,
      y: 60,
      width: 190,
      height: 68,
      lines: ['CHANGE Open', '-> Closed'],
    },
  ],
  edges: [
    {
      id: 'e1',
      source: 'n1',
      target: 'n2',
      kind: 'df',
      path: 'M 220 94 C 240 78 280 110 300 94',
      label: '2',
      title: 'Order: 2',
      weight: 2,
      object_type: 'Order',
      color: 'hsl(214 68% 38%)',
      directed: true,
      points: [
        { x: 220, y: 94 },
        { x: 260, y: 94 },
        { x: 300, y: 94 },
      ],
      label_x: 260,
      label_y: 86,
      object_types: [{ object_type: 'Order', weight: 2 }],
    },
  ],
};

export const traditionalProcessGraph: ProcessGraph = {
  ...processGraph,
  title: 'Object-Centric Directly-Follows Graph',
  subtitle: 'Flattened over selected object types with typed lifecycle edges',
  nodes: [
    processGraph.nodes[0],
    {
      ...processGraph.nodes[1],
      label: 'Create Order',
      kind: 'activity',
      count: 5,
      lines: ['Create Order'],
    },
  ],
  edges: [
    {
      ...processGraph.edges[0],
      source: 'n0',
      target: 'n1',
      weight: 5,
      label: '5',
      title: 'Order: 5',
    },
  ],
};
