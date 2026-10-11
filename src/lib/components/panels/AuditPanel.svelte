<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import type { AuditStats, SqlLogEntry } from '$lib/types';
  import {
    ShieldCheck,
    RefreshCw,
    Download,
    Trash2,
    Search,
    Clock,
    AlertTriangle,
    Database,
    FolderOpen,
  } from 'lucide-svelte';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { t } from '$lib/i18n/i18n.svelte';
  import { toasts } from '$lib/stores/toast.svelte';
  import { confirmDialog } from '$lib/stores/confirm.svelte';

  let entries = $state<SqlLogEntry[]>([]);
  let stats = $state<AuditStats | null>(null);
  let loading = $state(false);

  // Filters
  let filterText = $state('');
  let filterType = $state('');
  let errorsOnly = $state(false);
  let filterLimit = $state(500);
  let onlyCurrentConn = $state(false);

  let lastExport = $state<string | null>(null);
  let storageDegraded = $state(false);

  const STATEMENT_TYPES = ['SELECT', 'INSERT', 'UPDATE', 'DELETE', 'CREATE', 'DROP', 'ALTER', 'OTHER'];

  async function load() {
    loading = true;
    try {
      [entries, stats, storageDegraded] = await Promise.all([
        api.getAuditLog({
          limit: filterLimit,
          text: filterText || undefined,
          errorsOnly: errorsOnly || undefined,
          statementType: filterType || undefined,
          connectionId: onlyCurrentConn ? workspace.activeConnection?.id : undefined,
        }),
        api.getAuditStats(),
        api.isStorageDegraded(),
      ]);
      entries = entries.slice().reverse(); // newest first for display
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });

  async function exportAudit(format: 'csv' | 'json') {
    try {
      lastExport = await api.exportAuditLog(format);
    } catch (err: any) {
      toasts.error(t('grid.export_failed') + err);
    }
  }

  async function openExportsFolder() {
    try {
      const dir = await api.getDataDir('exports');
      await openPath(dir);
    } catch (err: any) {
      toasts.error(t('grid.open_exports_failed') + err);
    }
  }

  async function clearAudit() {
    const ok = await confirmDialog.ask({
      title: t('audit.clear_confirm'),
      confirmLabel: t('confirm.clear'),
      danger: true,
    });
    if (!ok) return;
    await api.clearAuditLog();
    await load();
  }

  function typeColor(t?: string): string {
    switch (t) {
      case 'SELECT': return 'text-sky-400';
      case 'INSERT': return 'text-emerald-400';
      case 'UPDATE': return 'text-amber-400';
      case 'DELETE': return 'text-rose-400';
      case 'CREATE':
      case 'DROP':
      case 'ALTER': return 'text-violet-400';
      default: return 'text-muted';
    }
  }

  function envBadge(env?: string): string {
    switch (env) {
      case 'production': return 'bg-rose-500/15 text-rose-400 border-rose-500/40';
      case 'staging': return 'bg-amber-500/15 text-amber-400 border-amber-500/40';
      default: return 'bg-emerald-500/15 text-emerald-400 border-emerald-500/40';
    }
  }
</script>

<div class="flex-1 flex flex-col h-full bg-app text-xs select-none overflow-hidden">
  <!-- Header -->
  <div class="h-10 border-b border-edge bg-panel flex items-center justify-between px-4">
    <div class="flex items-center gap-2 font-semibold text-fg">
      <ShieldCheck class="w-4 h-4 text-emerald-400" />
      <span>{t('audit.title')}</span>
      <span class="text-[10px] text-faint font-mono">{t('audit.subtitle')}</span>
    </div>
    <div class="flex items-center gap-1.5">
      <button
        class="flex items-center gap-1 px-2 py-1 rounded bg-elevated hover:bg-hover text-fg2 transition-colors"
        onclick={load}
        title={t('common.refresh')}
      >
        <RefreshCw class="w-3.5 h-3.5" />
      </button>
      <button
        class="flex items-center gap-1 px-2 py-1 rounded bg-elevated hover:bg-hover text-fg2 transition-colors"
        onclick={() => exportAudit('csv')}
        title={t('audit.export_csv_hint')}
      >
        <Download class="w-3.5 h-3.5 text-sky-400" /><span>CSV</span>
      </button>
      <button
        class="flex items-center gap-1 px-2 py-1 rounded bg-elevated hover:bg-hover text-fg2 transition-colors"
        onclick={() => exportAudit('json')}
        title={t('audit.export_json_hint')}
      >
        <Download class="w-3.5 h-3.5 text-violet-400" /><span>JSON</span>
      </button>
      <button
        class="flex items-center gap-1 px-2 py-1 rounded bg-elevated hover:bg-hover text-fg2 transition-colors"
        onclick={openExportsFolder}
        title={t('audit.open_exports')}
      >
        <FolderOpen class="w-3.5 h-3.5" />
      </button>
      <button
        class="flex items-center gap-1 px-2 py-1 rounded bg-rose-500/15 hover:bg-rose-500/25 text-rose-300 border border-rose-500/30 transition-colors"
        onclick={clearAudit}
        title={t('audit.clear_hint')}
      >
        <Trash2 class="w-3.5 h-3.5" />
      </button>
    </div>
  </div>

  {#if storageDegraded}
    <div class="px-4 py-1.5 bg-amber-950/40 border-b border-amber-900/40 text-amber-300 flex items-center gap-2">
      <AlertTriangle class="w-3.5 h-3.5 shrink-0" />
      <span>{t('audit.degraded')}</span>
    </div>
  {/if}

  <!-- Stats strip -->
  {#if stats}
    <div class="grid grid-cols-5 gap-2 p-3 border-b border-edge bg-subtle">
      <div class="rounded-lg border border-edge bg-panel p-2.5">
        <div class="text-[10px] uppercase tracking-wider text-faint">{t('audit.stat_total')}</div>
        <div class="text-lg font-bold text-fg font-mono">{stats.total}</div>
      </div>
      <div class="rounded-lg border border-edge bg-panel p-2.5">
        <div class="text-[10px] uppercase tracking-wider text-faint">{t('audit.stat_errors')}</div>
        <div class={`text-lg font-bold font-mono ${stats.error_count > 0 ? 'text-rose-400' : 'text-fg'}`}>
          {stats.error_count}
        </div>
      </div>
      <div class="rounded-lg border border-edge bg-panel p-2.5">
        <div class="text-[10px] uppercase tracking-wider text-faint">{t('audit.stat_avg')}</div>
        <div class="text-lg font-bold text-fg font-mono">{stats.avg_duration_ms.toFixed(1)}ms</div>
      </div>
      <div class="rounded-lg border border-edge bg-panel p-2.5">
        <div class="text-[10px] uppercase tracking-wider text-faint">{t('audit.stat_slowest')}</div>
        <div class="text-lg font-bold text-amber-400 font-mono">{stats.max_duration_ms.toFixed(1)}ms</div>
      </div>
      <div class="rounded-lg border border-edge bg-panel p-2.5 overflow-hidden">
        <div class="text-[10px] uppercase tracking-wider text-faint">{t('audit.stat_types')}</div>
        <div class="flex flex-wrap gap-x-2 gap-y-0.5 mt-1 font-mono text-[10px]">
          {#each Object.entries(stats.by_statement_type).sort((a, b) => b[1] - a[1]).slice(0, 6) as [t, n]}
            <span class={typeColor(t)}>{t}:{n}</span>
          {/each}
        </div>
      </div>
    </div>
  {/if}

  <!-- Filter bar -->
  <div class="h-10 border-b border-edge bg-panel flex items-center gap-2 px-3">
    <div class="relative flex items-center flex-1 max-w-md">
      <Search class="w-3 h-3 absolute left-2 text-faint" />
      <input
        type="text"
        placeholder={t('audit.filter_ph')}
        bind:value={filterText}
        onkeydown={(e) => e.key === 'Enter' && load()}
        class="w-full bg-subtle border border-edge rounded pl-7 pr-2 py-1 text-fg placeholder-faint focus:outline-none focus:border-sky-500 text-[11px] font-mono"
      />
    </div>
    <select
      bind:value={filterType}
      onchange={load}
      class="bg-subtle border border-edge rounded px-2 py-1 text-fg2 focus:outline-none focus:border-sky-500 text-[11px]"
    >
      <option value="">{t('audit.all_types')}</option>
      {#each STATEMENT_TYPES as t}
        <option value={t}>{t}</option>
      {/each}
    </select>
    <label class="flex items-center gap-1.5 text-muted cursor-pointer">
      <input type="checkbox" bind:checked={errorsOnly} onchange={load} class="rounded bg-elevated border-strong" />
      <span>{t('audit.errors_only')}</span>
    </label>
    <label class="flex items-center gap-1.5 text-muted cursor-pointer">
      <input type="checkbox" bind:checked={onlyCurrentConn} onchange={load} class="rounded bg-elevated border-strong" />
      <span>{t('audit.this_conn')}</span>
    </label>
    <select
      bind:value={filterLimit}
      onchange={load}
      class="bg-subtle border border-edge rounded px-2 py-1 text-fg2 focus:outline-none text-[11px]"
    >
      <option value={100}>{t('audit.last', { count: 100 })}</option>
      <option value={500}>{t('audit.last', { count: 500 })}</option>
      <option value={2000}>{t('audit.last', { count: 2000 })}</option>
    </select>
    <button class="px-2.5 py-1 rounded bg-elevated hover:bg-hover text-fg2" onclick={load}>{t('common.apply')}</button>
  </div>

  {#if lastExport}
    <div class="px-3 py-1.5 bg-emerald-950/30 border-b border-emerald-900/40 text-emerald-300 text-[11px] font-mono flex items-center gap-2">
      <Download class="w-3 h-3" /> {t('audit.exported', { path: lastExport })}
      <button class="underline" onclick={() => (lastExport = null)}>{t('common.dismiss')}</button>
    </div>
  {/if}

  <!-- Entries -->
  <div class="flex-1 overflow-y-auto p-2 space-y-1 font-mono text-[11px]">
    {#if loading}
      <div class="flex items-center justify-center py-10 text-faint gap-2">
        <div class="w-4 h-4 border-2 border-sky-500 border-t-transparent rounded-full animate-spin"></div>
        <span>{t('audit.loading')}</span>
      </div>
    {:else if entries.length === 0}
      <div class="text-faint italic p-6 text-center">{t('audit.empty')}</div>
    {:else}
      {#each entries as e (e.id)}
        <div
          class={`flex items-start justify-between gap-2 p-2 rounded border transition-colors ${
            e.is_error
              ? 'bg-rose-950/25 border-rose-900/40'
              : 'bg-subtle border-edge hover:bg-hover'
          }`}
        >
          <div class="flex items-start gap-2.5 overflow-hidden">
            <span class="text-faint shrink-0 text-[10px] w-[130px]">{e.timestamp}</span>
            <span class={`shrink-0 font-bold w-[58px] ${typeColor(e.statement_type)}`}>{e.statement_type ?? '—'}</span>
            <div class="flex items-center gap-1 text-sky-400 shrink-0 text-[10px] w-[62px]">
              <Clock class="w-2.5 h-2.5" /><span>{e.duration_ms.toFixed(1)}ms</span>
            </div>
            <div class="overflow-hidden">
              <div class="truncate select-text text-fg" title={e.query}>{e.query}</div>
              <div class="flex items-center gap-2 text-[10px] text-faint">
                {#if e.connection_name}
                  <span class="flex items-center gap-1"><Database class="w-2.5 h-2.5" />{e.connection_name}</span>
                {/if}
                {#if e.environment}
                  <span class={`px-1 rounded border text-[9px] uppercase ${envBadge(e.environment)}`}>{e.environment}</span>
                {/if}
                {#if e.affected_rows !== undefined && e.affected_rows !== null}
                  <span>{t('common.rows', { count: e.affected_rows })}</span>
                {/if}
                {#if e.error_message}
                  <span class="text-rose-400">{e.error_message}</span>
                {/if}
              </div>
            </div>
          </div>
          {#if e.is_error}
            <AlertTriangle class="w-3.5 h-3.5 text-rose-400 shrink-0 mt-0.5" />
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>
