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
      <div class="p-4 border-b border-edge flex items-center justify-between bg-subtle">
        <div class="flex items-center gap-2.5">
          <div class="p-1.5 rounded-md bg-accent-soft border border-accent-edge text-accent">
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
      <div class="p-4 border-b border-edge flex items-center gap-2 bg-panel">
        <input
          type="text"
          placeholder={t('gs.placeholder')}
          bind:value={keyword}
          onkeydown={(e) => e.key === 'Enter' && performSearch()}
          use:focusOnMount
          class="flex-1 bg-input border border-edge rounded-md px-3 py-2 text-fg placeholder-faint focus:outline-none focus:border-accent transition-colors font-mono"
        />
        <button
          class="px-4 py-2 rounded-md bg-accent-strong hover:opacity-95 text-white font-semibold transition-colors disabled:opacity-50"
          onclick={performSearch}
          disabled={isSearching}
        >
          {isSearching ? t('gs.searching') : t('gs.scan')}
        </button>
      </div>

      <!-- Results Area -->
      <div class="flex-1 overflow-y-auto p-4 space-y-3">
        {#if isSearching}
          <!-- Layout-matched skeletal loader -->
          <div class="space-y-2.5 animate-pulse" role="status" aria-label={t('gs.scanning')}>
            {#each Array(3) as _}
              <div class="p-3 rounded-lg bg-subtle border border-edge space-y-2">
                <div class="flex items-center justify-between">
                  <div class="h-3 w-48 rounded bg-elevated"></div>
                  <div class="h-4 w-20 rounded bg-elevated"></div>
                </div>
                <div class="h-5 w-full rounded bg-elevated"></div>
              </div>
            {/each}
          </div>
        {:else if hasSearched && searchResults.length === 0}
          <div class="py-10 flex flex-col items-center justify-center text-center gap-2 text-faint">
            <Search class="w-5 h-5 text-muted" />
            <div class="font-mono text-xs text-muted">{t('gs.no_match', { keyword })}</div>
          </div>
        {:else}
          {#each searchResults as res}
            <div class="p-3 rounded-lg bg-subtle border border-edge space-y-2">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2 font-semibold text-fg">
                  <Table class="w-3.5 h-3.5 text-accent" />
                  <span>{res.table_name}</span>
                  <span class="text-faint">→</span>
                  <span class="text-accent font-mono">{res.column_name}</span>
                  <span class="text-[10px] text-faint font-mono">({t('gs.matches', { count: res.match_count })})</span>
                </div>
                <button
                  class="flex items-center gap-1 px-2.5 py-1 rounded-md bg-elevated hover:bg-hover border border-edge text-accent text-[10px] font-medium"
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
