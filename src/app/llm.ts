export type LlmProviderId = 'openai' | 'openrouter' | 'grok' | 'mistral' | 'deepinfra';

export interface LlmProvider {
  id: LlmProviderId;
  name: string;
  baseUrl: string;
  defaultModel: string;
}

export interface LlmConfig {
  provider: LlmProviderId;
  model: string;
  apiKey: string;
}

export interface ChatMessage {
  role: 'system' | 'user' | 'assistant';
  content: string;
}

interface ChatCompletionResponse {
  choices?: Array<{
    message?: {
      content?: unknown;
    };
  }>;
  error?: {
    message?: string;
  };
}

export const LLM_PROVIDERS: LlmProvider[] = [
  {
    id: 'openai',
    name: 'OpenAI',
    baseUrl: 'https://api.openai.com/v1',
    defaultModel: 'gpt-5.4',
  },
  {
    id: 'openrouter',
    name: 'OpenRouter',
    baseUrl: 'https://openrouter.ai/api/v1',
    defaultModel: 'openai/gpt-5.4',
  },
  {
    id: 'grok',
    name: 'Grok',
    baseUrl: 'https://api.x.ai/v1',
    defaultModel: 'grok-4.3',
  },
  {
    id: 'mistral',
    name: 'Mistral',
    baseUrl: 'https://api.mistral.ai/v1',
    defaultModel: 'mistral-medium-3.5',
  },
  {
    id: 'deepinfra',
    name: 'DeepInfra',
    baseUrl: 'https://api.deepinfra.com/v1/openai',
    defaultModel: 'deepseek-ai/DeepSeek-V4-Flash',
  },
];

export const DEFAULT_LLM_PROVIDER = LLM_PROVIDERS[0];

export async function requestChatCompletion(
  config: LlmConfig,
  messages: ChatMessage[],
): Promise<string> {
  const provider = providerById(config.provider);
  const response = await fetch(`${provider.baseUrl}/chat/completions`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${config.apiKey}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      model: config.model || provider.defaultModel,
      messages,
    }),
  });
  const payload = (await readJsonResponse(response)) as ChatCompletionResponse;

  if (!response.ok) {
    throw new Error(payload.error?.message || `LLM request failed with HTTP ${response.status}`);
  }

  const content = payload.choices?.[0]?.message?.content;
  const text = chatContentToString(content).trim();
  if (!text) {
    throw new Error('The LLM response did not contain text.');
  }

  return text;
}

export function providerById(providerId: string): LlmProvider {
  return LLM_PROVIDERS.find((provider) => provider.id === providerId) ?? DEFAULT_LLM_PROVIDER;
}

function chatContentToString(content: unknown): string {
  if (typeof content === 'string') {
    return content;
  }

  if (Array.isArray(content)) {
    return content
      .map((part) => {
        if (typeof part === 'string') {
          return part;
        }
        if (part && typeof part === 'object' && 'text' in part) {
          return String((part as { text: unknown }).text ?? '');
        }
        return '';
      })
      .join('');
  }

  return '';
}

async function readJsonResponse(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text.trim()) {
    return {};
  }

  try {
    return JSON.parse(text);
  } catch {
    return {
      error: {
        message: text,
      },
    };
  }
}
