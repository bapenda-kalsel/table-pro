<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { getLocale, locales, setLocale, t } from '$lib/i18n/i18n.svelte';
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
  } from 'lucide-svelte';

  function toggleLanguage() {
    setLocale(getLocale() === 'en' ? 'id' : 'en');
  }

  const envColorMap = {
    local: 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30',
    staging: 'bg-amber-500/20 text-amber-400 border-amber-500/30',
    production: 'bg-rose-500/20 text-rose-400 border-rose-500/30 font-bold animate-pulse',
  };
</script>

<header class="h-10 border-b border-[#27272a] bg-[#18181b] flex items-center justify-between px-3 text-xs select-none drag-region">
  <!-- Left: App brand & active connection -->
  <div class="flex items-center gap-3">
    <div class="flex items-center gap-1.5 font-bold text-zinc-100 tracking-wide">
      <div class="w-4 h-4 rounded bg-gradient-to-tr from-sky-500 to-indigo-500 flex items-center justify-center text-[10px] text-white font-extrabold shadow-sm">
        T
      </div>
      <span>TablePro</span>
    </div>

    <!-- Active Connection Selector -->
    <button
      class="flex items-center gap-1.5 px-2 py-1 rounded bg-[#27272a]/70 hover:bg-[#3f3f46]/60 border border-[#3f3f46]/40 text-zinc-200 transition-colors"
      onclick={() => (workspace.isConnectionModalOpen = true)}
    >
      <Database class="w-3.5 h-3.5 text-sky-400" />
      <span class="font-medium max-w-[140px] truncate">
        {workspace.activeConnection ? workspace.activeConnection.name : t('titlebar.no_connection')}
      </span>
      {#if workspace.activeConnection}
        <span class={`text-[10px] uppercase px-1 py-0.2 rounded border ${envColorMap[workspace.activeConnection.environment]}`}>
          {workspace.activeConnection.environment}
        </span>
      {/if}
      <ChevronDown class="w-3 h-3 text-zinc-400" />
    </button>
  </div>

  <!-- Center: Quick Switcher / Open Anything (Cmd+P) -->
  <div class="flex items-center">
    <button
      class="flex items-center gap-2 px-3 py-1 rounded-md bg-[#27272a]/50 hover:bg-[#27272a] border border-[#3f3f46]/40 text-zinc-400 hover:text-zinc-200 transition-all text-xs w-64 justify-between"
      onclick={() => (workspace.isQuickSwitcherOpen = true)}
    >
      <div class="flex items-center gap-1.5">
        <Search class="w-3.5 h-3.5 text-zinc-400" />
        <span>{t('titlebar.search_placeholder')}</span>
      </div>
      <kbd class="px-1.5 py-0.5 rounded bg-[#18181b] border border-[#3f3f46]/60 text-[10px] text-zinc-400 font-mono">
        ⌘P
      </kbd>
    </button>
  </div>

  <!-- Right: HeidiSQL tools & Query Action -->
  <div class="flex items-center gap-1.5">
    <!-- New Query Tab -->
    <button
      class="flex items-center gap-1 px-2.5 py-1 rounded bg-sky-600/90 hover:bg-sky-500 text-white font-medium transition-colors shadow-sm"
      onclick={() => workspace.openQueryTab()}
      title={t('titlebar.new_query_hint')}
    >
      <Plus class="w-3.5 h-3.5" />
      <span>{t('titlebar.new_query')}</span>
    </button>

    <!-- Global Search in DB (HeidiSQL killer feature) -->
    <button
      class="p-1.5 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-100 transition-colors border border-transparent hover:border-[#3f3f46]/40"
      onclick={() => (workspace.isGlobalSearchModalOpen = true)}
      title={t('titlebar.find_text_hint')}
    >
      <Search class="w-3.5 h-3.5 text-amber-400" />
    </button>

    <!-- ER Diagram (MySQL Workbench feature) -->
    <button
      class="p-1.5 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-100 transition-colors border border-transparent hover:border-[#3f3f46]/40"
      onclick={() => workspace.openErdTab()}
      title={t('titlebar.erd_hint')}
    >
      <Network class="w-3.5 h-3.5 text-violet-300" />
    </button>

    <!-- Server Dashboard (MySQL Workbench feature) -->
    <button
      class="p-1.5 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-100 transition-colors border border-transparent hover:border-[#3f3f46]/40"
      onclick={() => workspace.openDashboardTab()}
      title={t('titlebar.dashboard_hint')}
    >
      <Gauge class="w-3.5 h-3.5 text-cyan-400" />
    </button>

    <!-- Query Audit Trail -->
    <button
      class="p-1.5 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-100 transition-colors border border-transparent hover:border-[#3f3f46]/40"
      onclick={() => workspace.openAuditTab()}
      title={t('titlebar.audit_hint')}
    >
      <ShieldCheck class="w-3.5 h-3.5 text-violet-400" />
    </button>

    <!-- Table Maintenance (HeidiSQL killer feature) -->
    <button
      class="p-1.5 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-100 transition-colors border border-transparent hover:border-[#3f3f46]/40"
      onclick={() => (workspace.isMaintenanceModalOpen = true)}
      title={t('titlebar.maintenance_hint')}
    >
      <Wrench class="w-3.5 h-3.5 text-emerald-400" />
    </button>

    <!-- Toggle Real-Time SQL Console (HeidiSQL killer feature) -->
    <button
      class={`p-1.5 rounded transition-colors border ${
        workspace.isLogConsoleOpen
          ? 'bg-sky-500/20 text-sky-400 border-sky-500/40'
          : 'hover:bg-[#27272a] text-zinc-400 hover:text-zinc-100 border-transparent hover:border-[#3f3f46]/40'
      }`}
      onclick={() => (workspace.isLogConsoleOpen = !workspace.isLogConsoleOpen)}
      title={t('titlebar.console_hint')}
    >
      <Terminal class="w-3.5 h-3.5" />
    </button>

    <!-- Language Switcher (EN / ID) -->
    <button
      class="p-1.5 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-100 transition-colors border border-transparent hover:border-[#3f3f46]/40 flex items-center gap-1"
      onclick={toggleLanguage}
      title={t('titlebar.language_hint')}
    >
      <Languages class="w-3.5 h-3.5 text-amber-300" />
      <span class="text-[10px] font-bold font-mono uppercase">{getLocale() === 'en' ? locales[0].flag : locales[1].flag}</span>
    </button>
  </div>
</header>
