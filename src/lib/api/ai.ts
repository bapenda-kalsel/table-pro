import { invoke, isTauri } from './client';
import type { AiConfigView, AiGenerateResult } from '../types';

export const aiApi = {
  async getAiConfig(): Promise<AiConfigView> {
    if (!isTauri) {
      return { provider: 'openai', model: 'gpt-4o-mini', baseUrl: null, hasKey: false, configured: false };
    }
    return invoke('ai_get_config');
  },

  async saveAiConfig(params: {
    provider: string;
    model: string;
    baseUrl?: string;
    apiKey?: string | null;
  }): Promise<AiConfigView> {
    if (!isTauri) {
      return { provider: params.provider, model: params.model, baseUrl: params.baseUrl ?? null, hasKey: true, configured: true };
    }
    return invoke('ai_save_config', {
      provider: params.provider,
      model: params.model,
      baseUrl: params.baseUrl ?? null,
      apiKey: params.apiKey ?? null,
    });
  },

  async aiGenerate(
    connectionId: string,
    prompt: string,
    currentSql?: string,
  ): Promise<AiGenerateResult> {
    if (!isTauri) {
      return { sql: `-- mock AI output for: ${prompt}\nSELECT * FROM users LIMIT 10;` };
    }
    return invoke('ai_generate', { connectionId, prompt, currentSql: currentSql ?? null });
  },
};
