<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import type { GlobalSearchResult } from '$lib/types';
  import { Search, X, Table, ArrowRight, Check } from 'lucide-svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { focusOnMount } from '$lib/utils/actions';
  import { toasts } from '$lib/stores/toast.svelte';

  let keyword = $state('');
  let isSearching = $state(false);
  let searchResults = $state<GlobalSearchResult[]>([]);
  let hasSearched = $state(false);

  async function performSearch() {
    if (!keyword.trim() || !workspace.activeConnection) return;
    isSearching = true;
    hasSearched = true;
    try {
      searchResults = await api.searchTextInDatabase(workspace.activeConnection.id, keyword);
    } catch (err: any) {
      toasts.error(t('gs.failed') + err);
    } finally {
      isSearching = false;
    }
  }

  function jumpToTable(tableName: string) {
    workspace.openTableTab(tableName);
    workspace.isGlobalSearchModalOpen = false;
  }
</script>

{#if workspace.isGlobalSearchModalOpen}
  <div class="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center z-50 p-4">
    <div class="w-[600px] max-h-[80vh] bg-panel border border-edge rounded-xl shadow-2xl flex flex-col text-xs overflow-hidden select-none">
      <!-- Modal Header -->
      <div class="p-4 border-b border-edge flex items-center justify-between">
        <div class="flex items-center gap-2">
          <div class="p-1.5 rounded bg-amber-500/20 text-amber-400">
            <Search class="w-4 h-4" />
          </div>
          <div>
            <h3 class="font-bold text-fg text-sm">{t('gs.title')}</h3>
            <p class="text-muted text-[11px]">{t('gs.subtitle')}</p>
          </div>
        </div>
        <button
          class="p-1 rounded hover:bg-hover text-muted hover:text-fg"
          onclick={() => (workspace.isGlobalSearchModalOpen = false)}
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Search Input -->
      <div class="p-4 border-b border-edge flex items-center gap-2 bg-subtle">
        <input
          type="text"
          placeholder={t('gs.placeholder')}
          bind:value={keyword}
          onkeydown={(e) => e.key === 'Enter' && performSearch()}
          use:focusOnMount
          class="flex-1 bg-elevated border border-edge rounded px-3 py-2 text-fg placeholder-faint focus:outline-none focus:border-amber-500 transition-colors font-mono"
        />
        <button
          class="px-4 py-2 rounded bg-amber-600 hover:bg-amber-500 text-white font-semibold transition-colors disabled:opacity-50"
          onclick={performSearch}
          disabled={isSearching}
        >
          {isSearching ? t('gs.searching') : t('gs.scan')}
        </button>
      </div>

      <!-- Results Area -->
      <div class="flex-1 overflow-y-auto p-4 space-y-3">
        {#if isSearching}
          <div class="flex items-center justify-center py-8 text-muted gap-2">
            <div class="w-4 h-4 border-2 border-amber-500 border-t-transparent rounded-full animate-spin"></div>
            <span>{t('gs.scanning')}</span>
          </div>
        {:else if hasSearched && searchResults.length === 0}
          <div class="py-8 text-center text-faint font-mono">
            {t('gs.no_match', { keyword })}
          </div>
        {:else}
          {#each searchResults as res}
            <div class="p-3 rounded-lg bg-subtle border border-edge space-y-2">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2 font-semibold text-fg">
                  <Table class="w-3.5 h-3.5 text-sky-400" />
                  <span>{res.table_name}</span>
                  <span class="text-faint">→</span>
                  <span class="text-amber-400 font-mono">{res.column_name}</span>
                  <span class="text-[10px] text-faint font-mono">({t('gs.matches', { count: res.match_count })})</span>
                </div>
                <button
                  class="flex items-center gap-1 px-2 py-0.5 rounded bg-elevated hover:bg-hover text-sky-400 text-[10px] font-medium"
                  onclick={() => jumpToTable(res.table_name)}
                >
                  <span>{t('gs.open_table')}</span>
                  <ArrowRight class="w-3 h-3" />
                </button>
              </div>

              <!-- Sample Matches -->
              <div class="space-y-1">
                {#each res.sample_rows as sample}
                  <div class="p-1.5 rounded bg-elevated text-fg2 font-mono text-[10px] truncate select-text">
                    {JSON.stringify(sample)}
                  </div>
                {/each}
              </div>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}
