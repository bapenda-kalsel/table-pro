<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { Search, Table, FileCode, Terminal, X } from 'lucide-svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { focusOnMount } from '$lib/utils/actions';

  let filterQuery = $state('');

  let filteredItems = $derived(
    workspace.tables.filter((t) =>
      t.name.toLowerCase().includes(filterQuery.toLowerCase())
    )
  );

  function openTable(tableName: string) {
    workspace.openTableTab(tableName);
    workspace.isQuickSwitcherOpen = false;
  }

  function handleKeyDown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'p') {
      e.preventDefault();
      workspace.isQuickSwitcherOpen = !workspace.isQuickSwitcherOpen;
    } else if (e.key === 'Escape') {
      workspace.isQuickSwitcherOpen = false;
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

{#if workspace.isQuickSwitcherOpen}
  <div class="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-start justify-center pt-24 z-50 p-4">
    <div class="w-[500px] bg-panel border border-edge rounded-xl shadow-2xl flex flex-col text-xs overflow-hidden select-none animate-in fade-in duration-100">
      <!-- Search Bar -->
      <div class="p-3 border-b border-edge flex items-center gap-2 bg-subtle">
        <Search class="w-4 h-4 text-sky-400" />
        <input
          type="text"
          placeholder={t('qs.placeholder')}
          bind:value={filterQuery}
          use:focusOnMount
          class="flex-1 bg-transparent text-fg placeholder-faint focus:outline-none text-sm"
        />
        <kbd class="px-1.5 py-0.5 rounded bg-elevated text-[10px] text-muted font-mono">ESC</kbd>
      </div>

      <!-- Quick Actions List -->
      <div class="max-h-72 overflow-y-auto p-1.5 space-y-1">
        <!-- New Query Action -->
        <div
          class="flex items-center gap-2.5 px-3 py-2 rounded-lg hover:bg-hover text-fg cursor-pointer transition-colors"
          onclick={() => {
            workspace.openQueryTab();
            workspace.isQuickSwitcherOpen = false;
          }}
          onkeydown={(e) => e.key === 'Enter' && workspace.openQueryTab()}
          tabindex="0"
          role="button"
        >
          <div class="p-1 rounded bg-sky-500/20 text-sky-400">
            <Terminal class="w-3.5 h-3.5" />
          </div>
          <div>
            <div class="font-semibold text-fg">{t('qs.new_query')}</div>
            <div class="text-[10px] text-faint">{t('qs.new_query_desc')}</div>
          </div>
        </div>

        <div class="px-2 pt-2 pb-1 text-[10px] font-semibold text-faint uppercase tracking-wider">
          {t('qs.tables', { count: filteredItems.length })}
        </div>

        {#each filteredItems as tbl}
          <div
            class="flex items-center justify-between px-3 py-2 rounded-lg hover:bg-hover text-fg2 hover:text-fg cursor-pointer transition-colors"
            onclick={() => openTable(tbl.name)}
            onkeydown={(e) => e.key === 'Enter' && openTable(tbl.name)}
            tabindex="0"
            role="button"
          >
            <div class="flex items-center gap-2">
              <Table class="w-3.5 h-3.5 text-sky-400" />
              <span class="font-medium">{tbl.name}</span>
            </div>
            {#if tbl.row_count_estimate !== undefined}
              <span class="text-[10px] text-faint font-mono">{t('common.rows', { count: tbl.row_count_estimate })}</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}
