<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import { t } from '$lib/i18n/i18n.svelte';
  import { Terminal, Trash2, X, Copy, Check, Clock } from 'lucide-svelte';

  let copiedId = $state<string | null>(null);

  function copyLog(id: string, text: string) {
    navigator.clipboard.writeText(text);
    copiedId = id;
    setTimeout(() => (copiedId = null), 2000);
  }

  async function clearLogs() {
    await api.clearSqlLogs();
    await workspace.refreshLogs();
  }
</script>

{#if workspace.isLogConsoleOpen}
  <div class="h-44 border-t border-[#27272a] bg-[#141417] flex flex-col text-xs select-none z-30">
    <!-- Header -->
    <div class="h-8 border-b border-[#27272a] bg-[#18181b] flex items-center justify-between px-3 text-zinc-400">
      <div class="flex items-center gap-2 font-medium">
        <Terminal class="w-3.5 h-3.5 text-sky-400" />
        <span class="text-zinc-200">{t('console.title')}</span>
        <span class="text-[10px] text-zinc-500 font-mono">({t('console.events', { count: workspace.sqlLogs.length })})</span>
      </div>

      <div class="flex items-center gap-1.5">
        <button
          class="p-1 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200 transition-colors"
          onclick={clearLogs}
          title={t('console.clear_hint')}
        >
          <Trash2 class="w-3 h-3" />
        </button>
        <button
          class="p-1 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200 transition-colors"
          onclick={() => (workspace.isLogConsoleOpen = false)}
          title={t('console.close_hint')}
        >
          <X class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>

    <!-- Logs List -->
    <div class="flex-1 overflow-y-auto p-2 space-y-1 font-mono text-[11px]">
      {#if workspace.sqlLogs.length === 0}
        <div class="text-zinc-600 italic p-2">{t('console.empty')}</div>
      {:else}
        {#each workspace.sqlLogs.slice().reverse() as log (log.id)}
          <div
            class={`flex items-start justify-between p-1.5 rounded transition-colors group ${
              log.is_error
                ? 'bg-rose-950/30 text-rose-300 border-l-2 border-rose-500'
                : 'hover:bg-[#1a1a1e] text-zinc-300'
            }`}
          >
            <div class="flex items-start gap-2.5 overflow-hidden">
              <span class="text-zinc-500 shrink-0 text-[10px]">{log.timestamp}</span>
              <div class="flex items-center gap-1 text-sky-400 shrink-0 text-[10px]">
                <Clock class="w-2.5 h-2.5" />
                <span>{log.duration_ms.toFixed(1)}ms</span>
              </div>
              <span class="truncate select-text font-mono text-zinc-200">{log.query}</span>
              {#if log.error_message}
                <span class="text-rose-400 text-[10px] font-sans truncate">({log.error_message})</span>
              {/if}
            </div>

            <button
              class="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-[#3f3f46] text-zinc-400 hover:text-zinc-100 transition-opacity shrink-0 ml-2"
              onclick={() => copyLog(log.id, log.query)}
              title={t('console.copy_hint')}
            >
              {#if copiedId === log.id}
                <Check class="w-3 h-3 text-emerald-400" />
              {:else}
                <Copy class="w-3 h-3" />
              {/if}
            </button>
          </div>
        {/each}
      {/if}
    </div>
  </div>
{/if}
