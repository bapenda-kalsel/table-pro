<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { Table, Terminal, FileCode, X, Plus, ShieldCheck, Network, Gauge } from 'lucide-svelte';

  // Right-click context menu (TablePlus/VSCode-style tab actions)
  let ctxMenu = $state<{ x: number; y: number; tabId: string } | null>(null);

  function openCtxMenu(e: MouseEvent, tabId: string) {
    e.preventDefault();
    e.stopPropagation();
    // Keep the menu inside the window.
    ctxMenu = {
      x: Math.min(e.clientX, window.innerWidth - 170),
      y: Math.min(e.clientY, window.innerHeight - 130),
      tabId,
    };
  }

  function runCtxAction(action: (tabId: string) => void) {
    if (!ctxMenu) return;
    action(ctxMenu.tabId);
    ctxMenu = null;
  }

  /// Tab close actions — gated by a confirm dialog when staged changes exist.
  function requestClose(tabId: string) {
    workspace.requestCloseTabs([tabId], () => workspace.closeTab(tabId));
  }

  function requestCloseOthers(tabId: string) {
    const ids = workspace.tabs.filter((x) => x.id !== tabId).map((x) => x.id);
    workspace.requestCloseTabs(ids, () => workspace.closeOtherTabs(tabId));
  }

  function requestCloseRight(tabId: string) {
    const idx = workspace.tabs.findIndex((x) => x.id === tabId);
    const ids = workspace.tabs.slice(idx + 1).map((x) => x.id);
    workspace.requestCloseTabs(ids, () => workspace.closeTabsToRight(tabId));
  }

  function requestCloseAll() {
    const ids = workspace.tabs.map((x) => x.id);
    workspace.requestCloseTabs(ids, () => workspace.closeAllTabs());
  }
</script>

<svelte:window
  onclick={() => (ctxMenu = null)}
  oncontextmenu={() => (ctxMenu = null)}
  onkeydown={(e) => e.key === 'Escape' && (ctxMenu = null)}
/>

<div class="h-9 bg-panel border-b border-edge flex items-center px-1 overflow-x-auto text-xs select-none">
  {#each workspace.tabs as tab (tab.id)}
    {@const stagedCount = tab.tableName ? workspace.getTotalStagedCount(tab.tableName) : 0}
    {@const isActive = workspace.activeTabId === tab.id}
    <div
      class={`group flex items-center gap-2 px-3 py-1.5 h-full border-r border-edge cursor-pointer transition-colors max-w-[200px] truncate ${
        isActive
          ? 'bg-app text-fg font-medium border-t-2 border-t-accent'
          : 'bg-panel text-muted hover:bg-elevated hover:text-fg'
      }`}
      onclick={() => (workspace.activeTabId = tab.id)}
      oncontextmenu={(e) => openCtxMenu(e, tab.id)}
      onkeydown={(e) => e.key === 'Enter' && (workspace.activeTabId = tab.id)}
      tabindex="0"
      role="tab"
      aria-selected={isActive}
    >
      <!-- Icon based on tab type (unified neutral/accent palette) -->
      {#if tab.type === 'table'}
        <Table class={`w-3.5 h-3.5 shrink-0 ${isActive ? 'text-accent' : 'text-muted'}`} />
      {:else if tab.type === 'structure'}
        <FileCode class={`w-3.5 h-3.5 shrink-0 ${isActive ? 'text-accent' : 'text-muted'}`} />
      {:else if tab.type === 'audit'}
        <ShieldCheck class={`w-3.5 h-3.5 shrink-0 ${isActive ? 'text-accent' : 'text-muted'}`} />
      {:else if tab.type === 'erd'}
        <Network class={`w-3.5 h-3.5 shrink-0 ${isActive ? 'text-accent' : 'text-muted'}`} />
      {:else if tab.type === 'dashboard'}
        <Gauge class={`w-3.5 h-3.5 shrink-0 ${isActive ? 'text-accent' : 'text-muted'}`} />
      {:else}
        <Terminal class={`w-3.5 h-3.5 shrink-0 ${isActive ? 'text-accent' : 'text-muted'}`} />
      {/if}

      <span class="truncate">{tab.title}</span>

      <!-- Staged changes badge -->
      {#if stagedCount > 0}
        <span class="w-2 h-2 rounded-full bg-warn shrink-0 animate-pulse" title={t('tabbar.pending', { count: stagedCount })}></span>
      {/if}

      <!-- Close Tab Button -->
      <button
        class="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-hover text-muted hover:text-fg transition-opacity ml-1"
        onclick={(e) => {
          e.stopPropagation();
          requestClose(tab.id);
        }}
        title={t('tabbar.close_hint')}
      >
        <X class="w-3 h-3" />
      </button>
    </div>
  {/each}

  <!-- Add Query Tab Button -->
  <button
    class="p-1.5 ml-1 rounded hover:bg-hover text-muted hover:text-fg transition-colors"
    onclick={() => workspace.openQueryTab()}
    title={t('tabbar.new_query_hint')}
  >
    <Plus class="w-3.5 h-3.5" />
  </button>
</div>

<!-- Tab Context Menu -->
{#if ctxMenu}
  {@const tabIdx = workspace.tabs.findIndex((x) => x.id === ctxMenu!.tabId)}
  <div
    class="fixed w-44 rounded-lg border border-edge bg-elevated shadow-2xl z-50 p-1 space-y-0.5 text-[11px]"
    style={`left: ${ctxMenu.x}px; top: ${ctxMenu.y}px`}
    oncontextmenu={(e) => e.preventDefault()}
    role="menu"
    tabindex="-1"
  >
    <button
      class="w-full text-left px-2 py-1 rounded text-fg hover:bg-hover transition-colors"
      onclick={() => runCtxAction(requestClose)}
      role="menuitem"
    >
      {t('tabbar.ctx_close')}
    </button>
    <button
      class="w-full text-left px-2 py-1 rounded text-fg hover:bg-hover transition-colors disabled:opacity-40 disabled:hover:bg-transparent"
      disabled={workspace.tabs.length <= 1}
      onclick={() => runCtxAction(requestCloseOthers)}
      role="menuitem"
    >
      {t('tabbar.ctx_close_others')}
    </button>
    <button
      class="w-full text-left px-2 py-1 rounded text-fg hover:bg-hover transition-colors disabled:opacity-40 disabled:hover:bg-transparent"
      disabled={tabIdx === -1 || tabIdx >= workspace.tabs.length - 1}
      onclick={() => runCtxAction(requestCloseRight)}
      role="menuitem"
    >
      {t('tabbar.ctx_close_right')}
    </button>
    <div class="border-t border-edge my-0.5"></div>
    <button
      class="w-full text-left px-2 py-1 rounded text-err hover:bg-err-bg transition-colors"
      onclick={() => runCtxAction(requestCloseAll)}
      role="menuitem"
    >
      {t('tabbar.ctx_close_all')}
    </button>
  </div>
{/if}
