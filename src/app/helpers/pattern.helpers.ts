import type { StatePattern } from '../ocel-wasm.service';
import type { PatternFilterRequest } from '../models/filter.models';
import type { PatternTab } from '../models/pattern.models';

export function patternFilterRequest(pattern: StatePattern): PatternFilterRequest {
  return {
    family: pattern.family as PatternTab,
    leading_object_type: pattern.leading_object_type,
    state: pattern.state ?? undefined,
    from_state: pattern.from_state ?? undefined,
    to_state: pattern.to_state ?? undefined,
    sequence: [...pattern.sequence],
    eo_edges: pattern.eo_edges.map(({ source, target }) => ({ source, target })),
    oo_edges: pattern.oo_edges.map(({ source, target }) => ({ source, target })),
  };
}

export function patternFilterLabel(pattern: PatternFilterRequest): string {
  if (pattern.family === 'inter') {
    return `${pattern.from_state ?? '?'} -> ${pattern.to_state ?? '?'} on ${pattern.leading_object_type}`;
  }
  return `${pattern.state ?? '?'} on ${pattern.leading_object_type}`;
}

export function wrapGraphLabel(label: string, maxLineLength: number, maxLines: number): string[] {
  const chunks = label
    .trim()
    .replace(/\s+\[/g, '\n[')
    .split('\n')
    .map((chunk) => chunk.replace(/\s+/g, ' ').trim())
    .filter(Boolean);
  const lines: string[] = [];

  for (const chunk of chunks) {
    let current = '';
    for (const word of chunk.split(' ')) {
      for (const part of splitLongWord(word, maxLineLength)) {
        const candidate = current ? `${current} ${part}` : part;
        if (candidate.length <= maxLineLength) {
          current = candidate;
        } else {
          lines.push(current);
          current = part;
        }
      }
    }
    if (current) {
      lines.push(current);
    }
  }

  if (lines.length <= maxLines) {
    return lines;
  }

  const trimmed = lines.slice(0, maxLines);
  trimmed[maxLines - 1] = `${trimmed[maxLines - 1].slice(0, maxLineLength - 3)}...`;
  return trimmed;
}

export function splitLongWord(word: string, maxLineLength: number): string[] {
  const parts: string[] = [];
  for (let index = 0; index < word.length; index += maxLineLength) {
    parts.push(word.slice(index, index + maxLineLength));
  }
  return parts.length > 0 ? parts : [''];
}
