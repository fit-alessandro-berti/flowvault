import type { CausalFitResult } from '../ocel-wasm.service';
import type { CausalFitGraph, CausalFitGraphEdge, CausalFitGraphNode, CausalNodeRole } from '../models/causal.models';
import { wrapGraphLabel } from './pattern.helpers';

export function causalFitGraph(fit: CausalFitResult): CausalFitGraph {
  const nodeWidth = 190;
  const nodeHeight = 74;
  const columns: CausalNodeRole[] = ['observable', 'latent', 'outcome'];
  const columnX: Record<CausalNodeRole, number> = {
    observable: 60,
    latent: 360,
    outcome: 660,
  };
  const nodes: CausalFitGraphNode[] = [];
  for (const role of columns) {
    const roleNodes = fit.nodes.filter((node) => node.role === role);
    roleNodes.forEach((node, index) => {
      nodes.push({
        id: node.id,
        label: node.label,
        role: node.role,
        x: columnX[role],
        y: 48 + index * 116,
        width: nodeWidth,
        height: nodeHeight,
        lines: wrapGraphLabel(node.label, 20, 3),
      });
    });
  }
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const maxRows = Math.max(
    1,
    ...columns.map((role) => fit.nodes.filter((node) => node.role === role).length),
  );
  const width = 900;
  const height = 70 + maxRows * 116;
  const edges = fit.edges
    .map((edge, index) => {
      const source = byId.get(edge.source);
      const target = byId.get(edge.target);
      if (!source || !target) {
        return null;
      }
      const x1 = source.x + source.width;
      const y1 = source.y + source.height / 2;
      const x2 = target.x;
      const y2 = target.y + target.height / 2;
      const midX = (x1 + x2) / 2;
      return {
        id: `causal-edge-${index}`,
        source,
        target,
        edge,
        path: `M ${x1} ${y1} C ${midX} ${y1}, ${midX} ${y2}, ${x2} ${y2}`,
        labelX: midX,
        labelY: (y1 + y2) / 2 - 6,
      };
    })
    .filter((edge): edge is CausalFitGraphEdge => edge !== null);

  return { width, height, nodes, edges };
}
