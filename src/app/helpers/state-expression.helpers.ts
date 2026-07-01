import { DEFAULT_LLM_PROVIDER, LlmConfig, providerById } from '../llm';

export const SAVED_STATE_PRESET_ID = '__saved_state_expression';

export const LLM_STATE_PRESET_ID = '__llm_state_expression';

export const LLM_CONFIG_STORAGE_KEY = 'flowvault.llmConfig';

export const STATE_EXPRESSION_STORAGE_KEY = 'flowvault.stateExpression';

export const DEFAULT_LLM_STATE_PROMPT = 'Give me a state expression for this object-centric event log.';

export const STATE_EXPRESSION_EXAMPLES = [
  `STATE state FOR LEADING OBJECT TYPE 'Invoice' AS CASE
  WHEN object.is_blocked = 'Yes' THEN 'Invoice Blocked'
  WHEN event.type LIKE '%Payment%' THEN 'Payment Execution'
  WHEN event.type LIKE '%Invoice%' THEN 'Invoice Handling'
  ELSE 'Procurement'
END`,
  `STATE state FOR LEADING OBJECT TYPE 'MAT' AS CASE
  WHEN event."Stock After" = 0 THEN 'Zero Stock'
  WHEN event."Stock After" < 30 THEN 'Low Stock'
  WHEN event."Stock After" >= 100 THEN 'High Stock'
  ELSE 'Available Stock'
END`,
  `STATE state FOR LEADING OBJECT TYPE 'orders' AS CASE
  WHEN event.type = 'item out of stock' THEN 'Stock Exception'
  WHEN event.type = 'reorder item' THEN 'Replenishment'
  WHEN event.type = 'payment reminder' THEN 'Payment Risk'
  ELSE 'Nominal'
END`,
];

export function loadLlmConfig(): LlmConfig {
  const stored = readStoredJson(LLM_CONFIG_STORAGE_KEY);
  if (!stored || typeof stored !== 'object') {
    return defaultLlmConfig();
  }

  const candidate = stored as Partial<LlmConfig>;
  const provider = providerById(String(candidate.provider ?? DEFAULT_LLM_PROVIDER.id));
  return {
    provider: provider.id,
    model: String(candidate.model ?? provider.defaultModel),
    apiKey: String(candidate.apiKey ?? ''),
  };
}

export function defaultLlmConfig(): LlmConfig {
  return {
    provider: DEFAULT_LLM_PROVIDER.id,
    model: DEFAULT_LLM_PROVIDER.defaultModel,
    apiKey: '',
  };
}

export function defaultStateQuery(leadingObjectType: string): string {
  return `STATE state FOR LEADING OBJECT TYPE '${leadingObjectType}' AS CASE
  WHEN event.type IS NOT NULL THEN 'Active'
  ELSE 'Other'
END`;
}

export function extractStateExpression(response: string): string {
  const fenced = response.match(/```(?:sql|text)?\s*([\s\S]*?)```/i);
  const candidate = (fenced?.[1] ?? response).trim();
  const start = candidate.toUpperCase().indexOf('STATE ');
  if (start >= 0) {
    return candidate.slice(start).trim();
  }
  return candidate;
}

export function readStoredString(key: string): string {
  try {
    return globalThis.localStorage?.getItem(key) ?? '';
  } catch {
    return '';
  }
}

export function writeStoredString(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch {
    return;
  }
}

export function readStoredJson(key: string): unknown {
  const stored = readStoredString(key);
  if (!stored) {
    return null;
  }

  try {
    return JSON.parse(stored);
  } catch {
    return null;
  }
}

export function writeStoredJson(key: string, value: unknown): void {
  try {
    globalThis.localStorage?.setItem(key, JSON.stringify(value));
  } catch {
    return;
  }
}
