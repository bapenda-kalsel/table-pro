<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { theme } from '$lib/stores/theme.svelte';
  import { checkForUpdates } from '$lib/stores/updater';
  import { getLocale, locales, setLocale, t } from '$lib/i18n/i18n.svelte';
  import Logo from '$lib/components/chrome/Logo.svelte';
  import {
    Database,
    Search,
    Terminal,
    Plus,
    Wrench,
    ShieldCheck,
    ChevronDown,
    Network,
    Gauge,
    Languages,
    Sun,
    Moon,
    ArrowDownToLine,
  } from 'lucide-svelte';

  function toggleLanguage() {
    setLocale(getLocale() === 'en' ? 'id' : 'en');
  }

  const envColorMap = {
    local: 'bg-ok-bg text-ok border-ok-edge',
    staging: 'bg-warn-bg text-warn border-warn-edge',
    production: 'bg-err-bg text-err border-err-edge font-bold animate-pulse',
  };
</script>

<header class="h-10 border-b border-edge bg-panel flex items-center justify-between px-3 text-xs select-none drag-region">
  <!-- Left: App brand & active connection -->
  <div class="flex items-center gap-3">
    <div class="flex items-center gap-2 font-semibold text-fg tracking-tight">
      <Logo class="w-5 h-5 shrink-0" />
      <span>TablePro</span>
    </div>

    <!-- Active Connection Selector -->
    <button
      class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-elevated hover:bg-hover border border-edge text-fg transition-colors"
      onclick={() => (workspace.isConnectionModalOpen = true)}
    >
      <Database class="w-3.5 h-3.5 text-accent" />
      <span class="font-medium max-w-[140px] truncate">
        {workspace.activeConnection ? workspace.activeConnection.name : t('titlebar.no_connection')}
      </span>
      {#if workspace.activeConnection}
        <span class={`text-[9px] font-mono uppercase px-1.5 py-0.5 rounded border ${envColorMap[workspace.activeConnection.environment]}`}>
          {workspace.activeConnection.environment}
        </span>
      {/if}
      <ChevronDown class="w-3 h-3 text-muted" />
    </button>
  </div>

  <!-- Center: Quick Switcher / Open Anything (Cmd+P) -->
  <div class="flex items-center">
    <button
      class="flex items-center gap-2 px-3 py-1 rounded-md bg-subtle hover:bg-hover border border-edge text-muted hover:text-fg transition-all text-xs w-64 justify-between"
      onclick={() => (workspace.isQuickSwitcherOpen = true)}
    >
      <div class="flex items-center gap-1.5">
        <Search class="w-3.5 h-3.5 text-faint" />
        <span>{t('titlebar.search_placeholder')}</span>
      </div>
      <kbd class="px-1.5 py-0.5 rounded bg-elevated border border-edge text-[10px] text-muted font-mono">
        ⌘P
      </kbd>
    </button>
  </div>

  <!-- Right: Workbench tools & Query Action -->
  <div class="flex items-center gap-1">
    <!-- New Query Tab -->
    <button
      class="flex items-center gap-1 px-2.5 py-1 rounded-md bg-accent-strong hover:opacity-95 text-white font-medium transition-all mr-1"
      onclick={() => workspace.openQueryTab()}
      title={t('titlebar.new_query_hint')}
    >
      <Plus class="w-3.5 h-3.5" />
      <span>{t('titlebar.new_query')}</span>
    </button>

    <!-- Global Search in DB -->
    <button
      class="p-1.5 rounded-md hover:bg-hover text-muted hover:text-fg transition-colors border border-transparent hover:border-edge"
      onclick={() => (workspace.isGlobalSearchModalOpen = true)}
      title={t('titlebar.find_text_hint')}
    >
      <Search class="w-3.5 h-3.5" />
    </button>

    <!-- ER Diagram -->
    <button
      class="p-1.5 rounded-md hover:bg-hover text-muted hover:text-fg transition-colors border border-transparent hover:border-edge"
      onclick={() => workspace.openErdTab()}
      title={t('titlebar.erd_hint')}
    >
      <Network class="w-3.5 h-3.5" />
    </button>

    <!-- Server Dashboard -->
    <button
      class="p-1.5 rounded-md hover:bg-hover text-muted hover:text-fg transition-colors border border-transparent hover:border-edge"
      onclick={() => workspace.openDashboardTab()}
      title={t('titlebar.dashboard_hint')}
    >
      <Gauge class="w-3.5 h-3.5" />
    </button>

    <!-- Query Audit Trail -->
    <button
      class="p-1.5 rounded-md hover:bg-hover text-muted hover:text-fg transition-colors border border-transparent hover:border-edge"
      onclick={() => workspace.openAuditTab()}
      title={t('titlebar.audit_hint')}
    >
      <ShieldCheck class="w-3.5 h-3.5" />
    </button>

    <!-- Table Maintenance -->
    <button
      class="p-1.5 rounded-md hover:bg-hover text-muted hover:text-fg transition-colors border border-transparent hover:border-edge"
      onclick={() => (workspace.isMaintenanceModalOpen = true)}
      title={t('titlebar.maintenance_hint')}
    >
      <Wrench class="w-3.5 h-3.5" />
    </button>

    <!-- Toggle Real-Time SQL Console -->
    <button
      class={`p-1.5 rounded-md transition-colors border ${
        workspace.isLogConsoleOpen
          ? 'bg-accent-soft text-accent border-accent-edge'
          : 'hover:bg-hover text-muted hover:text-fg border-transparent hover:border-edge'
      }`}
      onclick={() => (workspace.isLogConsoleOpen = !workspace.isLogConsoleOpen)}
      title={t('titlebar.console_hint')}
    >
      <Terminal class="w-3.5 h-3.5" />
    </button>

    <div class="w-px h-4 bg-edge mx-0.5"></div>

    <!-- Update Check -->
    <button
      class="p-1.5 rounded-md hover:bg-hover text-muted hover:text-fg transition-colors border border-transparent hover:border-edge"
      onclick={() => checkForUpdates(true)}
      title={t('titlebar.update_hint')}
    >
      <ArrowDownToLine class="w-3.5 h-3.5" />
    </button>

    <!-- Theme Toggle (Dark / Light) -->
    <button
      class="p-1.5 rounded-md hover:bg-hover text-muted hover:text-fg transition-colors border border-transparent hover:border-edge"
      onclick={() => theme.toggle()}
      title={t('titlebar.theme_hint')}
    >
      {#if theme.current === 'dark'}
        <Sun class="w-3.5 h-3.5" />
      {:else}
        <Moon class="w-3.5 h-3.5" />
      {/if}
    </button>

    <!-- Language Switcher (EN / ID) -->
    <button
      class="px-1.5 py-1 rounded-md hover:bg-hover text-muted hover:text-fg transition-colors border border-transparent hover:border-edge flex items-center gap-1"
      onclick={toggleLanguage}
      title={t('titlebar.language_hint')}
    >
      <Languages class="w-3.5 h-3.5" />
      <span class="text-[10px] font-semibold font-mono uppercase">{getLocale() === 'en' ? locales[0].flag : locales[1].flag}</span>
    </button>
  </div>
</header>
