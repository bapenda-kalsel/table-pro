<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import {
    Table,
    Eye,
    RefreshCw,
    Search,
    FileCode,
    FileUp,
    Plus,
    Terminal,
    Copy,
  } from 'lucide-svelte';
  import { qi } from '$lib/utils/sql';
  import { toasts } from '$lib/stores/toast.svelte';

  let searchTerm = $state('');
  let isRegex = $state(false);

  // Right-click context menu for table rows
  let ctxMenu = $state<{ x: number; y: number; table: string } | null>(null);

  function openCtxMenu(e: MouseEvent, table: string) {
    e.preventDefault();
    e.stopPropagation();
    ctxMenu = {
      x: Math.min(e.clientX, window.innerWidth - 170),
      y: Math.min(e.clientY, window.innerHeight - 190),
      table,
    };
  }

  function runCtxAction(action: (table: string) => void) {
    if (!ctxMenu) return;
    action(ctxMenu.table);
    ctxMenu = null;
  }

  function openData(table: string) {
    workspace.activeTable = table;
    workspace.openTableTab(table);
  }

  function openInQueryEditor(table: string) {
    workspace.openQueryTab(`SELECT * FROM ${qi(table)} LIMIT 100;`);
  }

  function copyTableName(table: string) {
    navigator.clipboard.writeText(table);
    toasts.success(t('common.copied'));
  }

  let filteredTables = $derived(
    workspace.tables.filter((tbl) => {
      if (!searchTerm) return true;
      if (isRegex) {
        try {
          const reg = new RegExp(searchTerm, 'i');
          return reg.test(tbl.name);
        } catch {
          return true;
        }
      }
      return tbl.name.toLowerCase().includes(searchTerm.toLowerCase());
    })
  );
</script>

<aside class="w-60 h-full border-r border-[#27272a] bg-[#141416] flex flex-col text-xs select-none">
  <!-- Sidebar Header / Search with Regex toggle (HeidiSQL Feature) -->
  <div class="p-2 border-b border-[#27272a] flex flex-col gap-1.5">
    <div class="flex items-center justify-between text-zinc-400 font-semibold uppercase tracking-wider text-[10px] px-1">
      <span>{t('sidebar.title')}</span>
      <div class="flex items-center gap-0.5">
        <button
          class="p-1 rounded hover:bg-[#27272a] text-zinc-400 hover:text-emerald-400 transition-colors"
          onclick={() => (workspace.isCreateTableModalOpen = true)}
          title={t('sidebar.create_table_hint')}
        >
          <Plus class="w-3 h-3" />
        </button>
        <button
          class="p-1 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200 transition-colors"
          onclick={() => (workspace.isImportCsvModalOpen = true)}
          title={t('sidebar.import_hint')}
        >
          <FileUp class="w-3 h-3" />
        </button>
        <button
          class="p-1 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200 transition-colors"
          onclick={() => workspace.refreshTables()}
          title={t('sidebar.refresh_hint')}
        >
          <RefreshCw class="w-3 h-3" />
        </button>
      </div>
    </div>

    <!-- Search input -->
    <div class="relative flex items-center">
      <Search class="w-3 h-3 absolute left-2 text-zinc-500" />
      <input
        type="text"
        placeholder={isRegex ? t('sidebar.filter_regex') : t('sidebar.filter_placeholder')}
        bind:value={searchTerm}
        class="w-full bg-[#1c1c1f] border border-[#2e2e33] rounded pl-7 pr-12 py-1 text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-sky-500 transition-colors text-[11px]"
      />
      <!-- Regex toggle badge -->
      <button
        type="button"
        class={`absolute right-1 text-[9px] font-mono px-1 py-0.5 rounded border transition-colors ${
          isRegex
            ? 'bg-sky-500/20 text-sky-400 border-sky-500/50 font-bold'
            : 'text-zinc-500 border-zinc-700 hover:text-zinc-300'
        }`}
        onclick={() => (isRegex = !isRegex)}
        title={t('sidebar.regex_hint')}
      >
        .*
      </button>
    </div>
  </div>

  <!-- Tables List -->
  <div class="flex-1 overflow-y-auto p-1 space-y-0.5">
    {#if workspace.tables.length === 0}
      <div class="p-4 text-center text-zinc-500">
        {#if workspace.isConnected}
          {t('sidebar.no_tables')}
        {:else}
          {t('sidebar.not_connected')}
        {/if}
      </div>
    {:else}
      {#each filteredTables as tbl (tbl.name)}
        <div
          class={`group flex items-center justify-between px-2 py-1.5 rounded cursor-pointer transition-colors ${
            workspace.activeTable === tbl.name
              ? 'bg-[#27272a] text-zinc-100 font-medium'
              : 'hover:bg-[#1f1f23] text-zinc-400 hover:text-zinc-200'
          }`}
          onclick={() => openData(tbl.name)}
          oncontextmenu={(e) => openCtxMenu(e, tbl.name)}
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              openData(tbl.name);
            }
          }}
          tabindex="0"
          role="button"
        >
          <div class="flex items-center gap-2 truncate">
            {#if tbl.table_type === 'view'}
              <Eye class="w-3.5 h-3.5 text-indigo-400 shrink-0" />
            {:else}
              <Table class="w-3.5 h-3.5 text-sky-400 shrink-0" />
            {/if}
            <span class="truncate">{tbl.name}</span>
          </div>

          <div class="flex items-center gap-1.5">
            {#if tbl.row_count_estimate !== undefined && tbl.row_count_estimate !== null}
              <span class="text-[10px] text-zinc-500 font-mono">
                {tbl.row_count_estimate}
              </span>
            {/if}

            <!-- Structure icon on hover -->
            <button
              class="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-[#3f3f46] text-zinc-400 hover:text-amber-400 transition-opacity"
              title={t('sidebar.structure_hint')}
              onclick={(e) => {
                e.stopPropagation();
                workspace.openStructureTab(tbl.name);
              }}
            >
              <FileCode class="w-3 h-3" />
            </button>
          </div>
        </div>
      {/each}
    {/if}
  </div>

  <!-- Bottom quick stats -->
  <div class="p-2 border-t border-[#27272a] text-[10px] text-zinc-500 flex justify-between items-center bg-[#18181b]/50">
    <span>{t('sidebar.object_count', { count: filteredTables.length })}</span>
    <span>{workspace.serverVersion || t('sidebar.default_engine')}</span>
  </div>
</aside>

<!-- Table Context Menu -->
{#if ctxMenu}
  <div
    class="fixed w-44 rounded-md border border-[#27272a] bg-[#1c1c20] shadow-2xl z-50 p-0.5 space-y-px text-[11px]"
    style={`left: ${ctxMenu.x}px; top: ${ctxMenu.y}px`}
    oncontextmenu={(e) => e.preventDefault()}
    role="menu"
    tabindex="-1"
  >
    <button
      class="w-full text-left px-2 py-1 rounded text-zinc-200 hover:bg-[#27272a] transition-colors flex items-center gap-1.5"
      onclick={() => runCtxAction(openData)}
      role="menuitem"
    >
      <Table class="w-3 h-3 text-sky-400" /> {t('sidebar.ctx_open')}
    </button>
    <button
      class="w-full text-left px-2 py-1 rounded text-zinc-200 hover:bg-[#27272a] transition-colors flex items-center gap-1.5"
      onclick={() => runCtxAction((tb) => workspace.openStructureTab(tb))}
      role="menuitem"
    >
      <FileCode class="w-3 h-3 text-amber-400" /> {t('sidebar.ctx_structure')}
    </button>
    <button
      class="w-full text-left px-2 py-1 rounded text-zinc-200 hover:bg-[#27272a] transition-colors flex items-center gap-1.5"
      onclick={() => runCtxAction(openInQueryEditor)}
      role="menuitem"
    >
      <Terminal class="w-3 h-3 text-emerald-400" /> {t('sidebar.ctx_query')}
    </button>
    <div class="border-t border-[#27272a] my-0.5"></div>
    <button
      class="w-full text-left px-2 py-1 rounded text-zinc-200 hover:bg-[#27272a] transition-colors flex items-center gap-1.5"
      onclick={() => runCtxAction(copyTableName)}
      role="menuitem"
    >
      <Copy class="w-3 h-3 text-zinc-400" /> {t('sidebar.ctx_copy')}
    </button>
    <button
      class="w-full text-left px-2 py-1 rounded text-zinc-200 hover:bg-[#27272a] transition-colors flex items-center gap-1.5"
      onclick={() => {
        ctxMenu = null;
        workspace.refreshTables();
      }}
      role="menuitem"
    >
      <RefreshCw class="w-3 h-3 text-zinc-400" /> {t('sidebar.ctx_refresh')}
    </button>
  </div>
{/if}

<svelte:window
  onclick={() => (ctxMenu = null)}
  oncontextmenu={() => (ctxMenu = null)}
  onkeydown={(e) => e.key === 'Escape' && (ctxMenu = null)}
/>
