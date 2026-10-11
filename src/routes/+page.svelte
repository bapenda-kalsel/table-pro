<script lang="ts">
  import { onMount } from 'svelte';
  import { workspace } from '$lib/stores/workspace.svelte';
  import TitleBar from '$lib/components/chrome/TitleBar.svelte';
  import Sidebar from '$lib/components/chrome/Sidebar.svelte';
  import TabBar from '$lib/components/chrome/TabBar.svelte';
  import SqlLogConsole from '$lib/components/chrome/SqlLogConsole.svelte';
  import DataGrid from '$lib/components/panels/DataGrid.svelte';
  import QueryEditor from '$lib/components/panels/QueryEditor.svelte';
  import StructureViewer from '$lib/components/panels/StructureViewer.svelte';
  import AuditPanel from '$lib/components/panels/AuditPanel.svelte';
  import ErDiagram from '$lib/components/panels/ErDiagram.svelte';
  import DashboardPanel from '$lib/components/panels/DashboardPanel.svelte';
  import QuickSwitcherModal from '$lib/components/modals/QuickSwitcherModal.svelte';
  import ConnectionModal from '$lib/components/modals/ConnectionModal.svelte';
  import CodeReviewModal from '$lib/components/modals/CodeReviewModal.svelte';
  import GlobalSearchModal from '$lib/components/modals/GlobalSearchModal.svelte';
  import MaintenanceModal from '$lib/components/modals/MaintenanceModal.svelte';
  import ImportCsvModal from '$lib/components/modals/ImportCsvModal.svelte';
  import CreateTableModal from '$lib/components/modals/CreateTableModal.svelte';
  import AiSettingsModal from '$lib/components/modals/AiSettingsModal.svelte';
  import ConfirmDialog from '$lib/components/modals/ConfirmDialog.svelte';
  import Toasts from '$lib/components/chrome/Toasts.svelte';
  import { Plus, Database, Terminal } from 'lucide-svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  onMount(async () => {
    await workspace.init();
  });

  let activeTab = $derived(
    workspace.tabs.find((t) => t.id === workspace.activeTabId)
  );

  function handleKeyDown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 't') {
      e.preventDefault();
      workspace.openQueryTab();
    } else if ((e.metaKey || e.ctrlKey) && e.key === 'w') {
      e.preventDefault();
      if (workspace.activeTabId) {
        const tabId = workspace.activeTabId;
        workspace.requestCloseTabs([tabId], () => workspace.closeTab(tabId));
      }
    } else if ((e.metaKey || e.ctrlKey) && e.key === 'j') {
      e.preventDefault();
      workspace.isLogConsoleOpen = !workspace.isLogConsoleOpen;
    } else if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'F') {
      e.preventDefault();
      workspace.isGlobalSearchModalOpen = true;
    } else if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'A') {
      e.preventDefault();
      workspace.openAuditTab();
    } else if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'E') {
      e.preventDefault();
      workspace.openErdTab();
    } else if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'D') {
      e.preventDefault();
      workspace.openDashboardTab();
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="h-screen w-screen flex flex-col bg-app text-fg overflow-hidden select-none">
  <!-- Top: TablePro Native Title Bar -->
  <TitleBar />

  <!-- Center: Workspace (Sidebar + Main Content) -->
  <div class="flex-1 flex overflow-hidden">
    <!-- Sidebar -->
    <Sidebar />

    <!-- Main Content Area -->
    <main class="flex-1 flex flex-col h-full overflow-hidden bg-app">
      <!-- Tab Bar -->
      <TabBar />

      <!-- Active Tab Content -->
      <div class="flex-1 flex flex-col overflow-hidden relative">
        {#if !activeTab}
          <!-- Empty Workspace State -->
          <div class="flex-1 flex flex-col items-center justify-center text-center p-8 text-faint space-y-4">
            <div class="w-16 h-16 rounded-2xl bg-gradient-to-tr from-sky-500/10 to-indigo-500/10 border border-sky-500/20 flex items-center justify-center text-sky-400">
              <Database class="w-8 h-8" />
            </div>
            <div>
              <h2 class="text-base font-semibold text-fg">{t('welcome.title')}</h2>
              <p class="text-xs text-faint mt-1 max-w-sm">
                {t('welcome.sub_pre')} <kbd class="px-1.5 py-0.5 rounded bg-elevated text-fg2 font-mono">⌘P</kbd> {t('welcome.sub_post')}
              </p>
            </div>
            <div class="flex items-center gap-3">
              <button
                class="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-sky-600 hover:bg-sky-500 text-white font-medium text-xs shadow-md transition-colors"
                onclick={() => workspace.openQueryTab()}
              >
                <Terminal class="w-3.5 h-3.5" />
                <span>{t('welcome.new_query')}</span>
              </button>
            </div>
          </div>
        {:else}
          <!-- Keyed per tab + connection: a panel can never carry internal
               state from a previous tab or a replaced session. -->
          {#key activeTab.id + ':' + (activeTab.connectionId ?? '')}
            {#if activeTab.type === 'table' && activeTab.tableName}
              <DataGrid tableName={activeTab.tableName} />
            {:else if activeTab.type === 'structure' && activeTab.tableName}
              <StructureViewer tableName={activeTab.tableName} />
            {:else if activeTab.type === 'audit'}
              <AuditPanel />
            {:else if activeTab.type === 'erd'}
              <ErDiagram />
            {:else if activeTab.type === 'dashboard'}
              <DashboardPanel />
            {:else if activeTab.type === 'query'}
              <QueryEditor tab={activeTab} />
            {/if}
          {/key}
        {/if}
      </div>

      <!-- Real-Time Bottom SQL Log Console (HeidiSQL Feature) -->
      <SqlLogConsole />
    </main>
  </div>

  <!-- Global Modals -->
  <QuickSwitcherModal />
  <ConnectionModal />
  <CodeReviewModal />
  <GlobalSearchModal />
  <MaintenanceModal />
  <ImportCsvModal />
  <CreateTableModal />
  <AiSettingsModal />
  <ConfirmDialog />
  <Toasts />
</div>
