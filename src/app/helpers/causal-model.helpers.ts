import type { CausalModelEdge, CausalModelNode, CausalNodeRole, CausalOperation } from '../models/causal.models';

export function nextCausalNodeId(role: CausalNodeRole, nodes: CausalModelNode[]): string {
  const prefix = role === 'observable' ? 'obs' : role === 'outcome' ? 'out' : 'lat';
  const existing = new Set(nodes.map((node) => node.id));
  for (let index = 1; ; index += 1) {
    const id = `${prefix}-${index}`;
    if (!existing.has(id)) {
      return id;
    }
  }
}

export function parseCausalModelSuggestion(
  response: string,
  availableFeatures: string[],
): { nodes: CausalModelNode[]; edges: CausalModelEdge[] } {
  const parsed = readJsonObject(extractJsonPayload(response));
  const rawNodes = Array.isArray(parsed['nodes']) ? parsed['nodes'] : [];
  const rawEdges = Array.isArray(parsed['edges']) ? parsed['edges'] : [];
  const featureSet = new Set(availableFeatures);
  const nodes: CausalModelNode[] = [];
  const originalToNewId = new Map<string, string>();

  for (const rawNode of rawNodes) {
    if (!rawNode || typeof rawNode !== 'object') {
      continue;
    }
    const record = rawNode as Record<string, unknown>;
    const role = normalizeCausalRole(record['role']);
    if (!role) {
      continue;
    }
    const originalId = String(record['id'] ?? record['label'] ?? `${role}-${nodes.length + 1}`);
    const label = String(record['label'] ?? record['name'] ?? originalId).trim();
    if (role === 'latent') {
      const node: CausalModelNode = {
        id: nextCausalNodeId('latent', nodes),
        label: label || `Latent ${nodes.filter((node) => node.role === 'latent').length + 1}`,
        role,
        operation: 'identity',
      };
      nodes.push(node);
      originalToNewId.set(originalId, node.id);
      originalToNewId.set(node.label, node.id);
      continue;
    }

    const feature = String(record['feature'] ?? '').trim();
    if (!featureSet.has(feature)) {
      continue;
    }
    const operation = normalizeCausalOperation(record['operation']);
    const node: CausalModelNode = {
      id: nextCausalNodeId(role, nodes),
      label: label || causalFeatureLabel(feature),
      role,
      feature,
      operation,
    };
    nodes.push(node);
    originalToNewId.set(originalId, node.id);
    originalToNewId.set(node.label, node.id);
  }

  const edges: CausalModelEdge[] = [];
  for (const rawEdge of rawEdges) {
    if (!rawEdge || typeof rawEdge !== 'object') {
      continue;
    }
    const record = rawEdge as Record<string, unknown>;
    const source = originalToNewId.get(String(record['source'] ?? ''));
    const target = originalToNewId.get(String(record['target'] ?? ''));
    if (!source || !target || !canAddCausalEdge(nodes, edges, source, target)) {
      continue;
    }
    edges.push({ source, target });
  }

  if (nodes.length === 0) {
    throw new Error('The LLM did not return any usable causal model nodes.');
  }
  return { nodes, edges };
}

export function extractJsonPayload(response: string): string {
  const fenced = response.match(/```(?:json)?\s*([\s\S]*?)```/i);
  const candidate = (fenced?.[1] ?? response).trim();
  const start = candidate.indexOf('{');
  const end = candidate.lastIndexOf('}');
  if (start >= 0 && end > start) {
    return candidate.slice(start, end + 1);
  }
  return candidate;
}

export function readJsonObject(jsonText: string): Record<string, unknown> {
  const parsed = JSON.parse(jsonText) as unknown;
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('The LLM response was not a JSON object.');
  }
  return parsed as Record<string, unknown>;
}

export function normalizeCausalRole(value: unknown): CausalNodeRole | null {
  const role = String(value ?? '')
    .trim()
    .toLowerCase();
  if (role === 'observable' || role === 'latent' || role === 'outcome') {
    return role;
  }
  return null;
}

export function normalizeCausalOperation(value: unknown): CausalOperation {
  const operation = String(value ?? '')
    .trim()
    .toLowerCase();
  if (operation === 'log_10' || operation === 'log10') {
    return 'log10';
  }
  if (operation === 'ln' || operation === 'loge' || operation === 'log_e') {
    return 'log_e';
  }
  if (operation === 'sqrt' || operation === 'square_root') {
    return 'sqrt';
  }
  return 'identity';
}

export function causalFeatureLabel(feature: string): string {
  return feature
    .replace(/^activity\./, '')
    .replace(/^attribute\./, '')
    .replace(/^related_objects\./, 'Related ')
    .replace(/=/g, ' = ');
}

export function canAddCausalEdge(
  nodes: CausalModelNode[],
  edges: CausalModelEdge[],
  source: string,
  target: string,
): boolean {
  if (source === target || edges.some((edge) => edge.source === source && edge.target === target)) {
    return false;
  }
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const sourceNode = byId.get(source);
  const targetNode = byId.get(target);
  if (!sourceNode || !targetNode || !isLegalCausalEdge(sourceNode, targetNode)) {
    return false;
  }
  return !causalGraphHasCycle(nodes, [...edges, { source, target }]);
}

export function isLegalCausalEdge(source: CausalModelNode, target: CausalModelNode): boolean {
  return (
    (source.role === 'observable' && target.role === 'latent') ||
    (source.role === 'latent' && target.role === 'latent') ||
    (source.role === 'latent' && target.role === 'outcome')
  );
}

export function pruneCausalEdges(nodes: CausalModelNode[], edges: CausalModelEdge[]): CausalModelEdge[] {
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const pruned: CausalModelEdge[] = [];
  for (const edge of edges) {
    const source = byId.get(edge.source);
    const target = byId.get(edge.target);
    if (!source || !target || !isLegalCausalEdge(source, target)) {
      continue;
    }
    if (!causalGraphHasCycle(nodes, [...pruned, edge])) {
      pruned.push(edge);
    }
  }
  return pruned;
}

export function causalGraphHasCycle(nodes: CausalModelNode[], edges: CausalModelEdge[]): boolean {
  const indegree = new Map(nodes.map((node) => [node.id, 0]));
  const outgoing = new Map(nodes.map((node) => [node.id, [] as string[]]));
  for (const edge of edges) {
    if (!indegree.has(edge.source) || !indegree.has(edge.target)) {
      continue;
    }
    indegree.set(edge.target, (indegree.get(edge.target) ?? 0) + 1);
    outgoing.get(edge.source)?.push(edge.target);
  }

  const ready = nodes.filter((node) => (indegree.get(node.id) ?? 0) === 0).map((node) => node.id);
  let visited = 0;
  while (ready.length > 0) {
    const nodeId = ready.shift() ?? '';
    visited += 1;
    for (const target of outgoing.get(nodeId) ?? []) {
      const next = (indegree.get(target) ?? 0) - 1;
      indegree.set(target, next);
      if (next === 0) {
        ready.push(target);
      }
    }
  }
  return visited !== nodes.length;
}
