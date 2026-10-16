<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import { t } from '$lib/i18n/i18n.svelte';
  import { toasts } from '$lib/stores/toast.svelte';
  import { X, Sparkles, KeyRound } from 'lucide-svelte';
  import type { AiConfigView } from '$lib/types';

  const PROVIDERS = [
    { id: 'openai', label: 'OpenAI-compatible', defaultModel: 'gpt-4o-mini', urlHint: 'https://api.openai.com/v1' },
    { id: 'anthropic', label: 'Anthropic', defaultModel: 'claude-haiku-4-5', urlHint: 'https://api.anthropic.com/v1' },
    { id: 'gemini', label: 'Google Gemini', defaultModel: 'gemini-2.0-flash', urlHint: 'https://generativelanguage.googleapis.com/v1beta' },
  ];

  let provider = $state('openai');
  let model = $state('');
  let baseUrl = $state('');
  let apiKey = $state('');
  let clearKey = $state(false);
  let current = $state<AiConfigView | null>(null);
  let saving = $state(false);
  let errorMsg = $state<string | null>(null);

  let providerMeta = $derived(PROVIDERS.find((p) => p.id === provider) ?? PROVIDERS[0]);

  $effect(() => {
    if (workspace.isAiSettingsOpen) {
      load();
    }
  });

  async function load() {
    try {
      current = await api.getAiConfig();
      provider = current.provider;
      model = current.model;
      baseUrl = current.baseUrl ?? '';
      apiKey = '';
      clearKey = false;
      errorMsg = null;
    } catch (err: any) {
      errorMsg = String(err);
    }
  }

  function selectProvider(id: string) {
    provider = id;
    const meta = PROVIDERS.find((p) => p.id === id);
    if (meta && (!model || PROVIDERS.some((p) => p.defaultModel === model))) {
      model = meta.defaultModel;
    }
  }

  async function save() {
    saving = true;
    errorMsg = null;
    try {
      current = await api.saveAiConfig({
        provider,
        model: model.trim() || providerMeta.defaultModel,
        baseUrl: baseUrl.trim() || undefined,
        apiKey: clearKey ? '' : apiKey.trim() || undefined,
      });
      apiKey = '';
      clearKey = false;
      toasts.success(t('ai.saved'));
      workspace.isAiSettingsOpen = false;
    } catch (err: any) {
      errorMsg = String(err);
    } finally {
      saving = false;
    }
  }
</script>

{#if workspace.isAiSettingsOpen}
  <div class="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center z-50 p-4">
    <div class="w-[440px] bg-panel border border-edge rounded-xl shadow-2xl flex flex-col text-xs select-none">
      <div class="p-4 border-b border-edge flex items-center justify-between bg-subtle rounded-t-xl">
        <div class="flex items-center gap-2 font-semibold text-fg">
          <Sparkles class="w-4 h-4 text-accent" />
          {t('ai.settings_title')}
        </div>
        <button class="text-muted hover:text-fg" onclick={() => (workspace.isAiSettingsOpen = false)}>
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="p-4 space-y-4">
        <!-- Provider -->
        <div class="space-y-1.5">
          <label class="text-[10px] text-muted uppercase font-semibold block" for="ai-provider">{t('ai.provider')}</label>
          <select
            id="ai-provider"
            bind:value={provider}
            onchange={(e) => selectProvider((e.target as HTMLSelectElement).value)}
            class="w-full bg-input border border-edge rounded-md px-2.5 py-1.5 text-fg focus:outline-none focus:border-accent"
          >
            {#each PROVIDERS as p}
              <option value={p.id}>{p.label}</option>
            {/each}
          </select>
        </div>

        <!-- Model -->
        <div class="space-y-1.5">
          <label class="text-[10px] text-muted uppercase font-semibold block" for="ai-model">{t('ai.model')}</label>
          <input
            id="ai-model"
            type="text"
            bind:value={model}
            placeholder={providerMeta.defaultModel}
            class="w-full bg-input border border-edge rounded-md px-2.5 py-1.5 text-fg placeholder-faint focus:outline-none focus:border-accent font-mono"
          />
        </div>

        <!-- Base URL (OpenAI-compatible / self-hosted) -->
        <div class="space-y-1.5">
          <label class="text-[10px] text-muted uppercase font-semibold block" for="ai-baseurl">{t('ai.base_url')}</label>
          <input
            id="ai-baseurl"
            type="text"
            bind:value={baseUrl}
            placeholder={providerMeta.urlHint}
            class="w-full bg-input border border-edge rounded-md px-2.5 py-1.5 text-fg placeholder-faint focus:outline-none focus:border-accent font-mono"
          />
          <div class="text-[10px] text-faint">{t('ai.base_url_hint')}</div>
        </div>

        <!-- API key -->
        <div class="space-y-1.5">
          <label class="text-[10px] text-muted uppercase font-semibold flex items-center gap-1" for="ai-key">
            <KeyRound class="w-3 h-3 text-accent" /> {t('ai.api_key')}
          </label>
          <input
            id="ai-key"
            type="password"
            bind:value={apiKey}
            placeholder={current?.hasKey ? t('ai.key_stored') : t('ai.key_placeholder')}
            class="w-full bg-input border border-edge rounded-md px-2.5 py-1.5 text-fg placeholder-faint focus:outline-none focus:border-accent font-mono"
          />
          <div class="flex items-center justify-between">
            <span class="text-[10px] text-faint">{t('ai.key_hint')}</span>
            {#if current?.hasKey}
              <label class="flex items-center gap-1 text-[10px] text-err cursor-pointer">
                <input type="checkbox" bind:checked={clearKey} class="rounded accent-accent" />
                {t('ai.clear_key')}
              </label>
            {/if}
          </div>
        </div>

        {#if errorMsg}
          <div class="text-err text-[11px] bg-err-bg border border-err-edge rounded-md px-2.5 py-1.5">{errorMsg}</div>
        {/if}
      </div>

      <div class="p-3 border-t border-edge flex justify-end gap-2 bg-subtle rounded-b-xl">
        <button
          class="px-3 py-1.5 rounded-md bg-elevated hover:bg-hover border border-edge text-fg2 transition-colors"
          onclick={() => (workspace.isAiSettingsOpen = false)}
        >
          {t('common.cancel')}
        </button>
        <button
          class="px-3.5 py-1.5 rounded-md bg-accent-strong hover:opacity-95 text-white font-medium transition-colors disabled:opacity-50"
          onclick={save}
          disabled={saving}
        >
          {saving ? t('common.saving') : t('common.save')}
        </button>
      </div>
    </div>
  </div>
{/if}
