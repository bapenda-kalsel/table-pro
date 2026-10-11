<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import type { AuditStats, DatabaseStats } from '$lib/types';
  import {
    Gauge, RefreshCw, Database, HardDrive, Table, Eye, KeyRound, Zap, Rows3,
    FileText, ShieldCheck, Clock, Layers,
  } from 'lucide-svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  let stats = $state<DatabaseStats | null>(null);
  let audit = $state<AuditStats | null>(null);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  async function load() {
    if (!workspace.activeConnection) return;
    loading = true;
    loadError = null;
    try {
      [stats, audit] = await Promise.all([
        api.getDatabaseStats(workspace.activeConnection.id),
        api.getAuditStats(),
      ]);
    } catch (err: any) {
      loadError = String(err);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (workspace.activeConnection) load();
  });

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(2)} MB`;
    return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }

  let maxTableRows = $derived(
    Math.max(1, ...workspace.tables.map((t) => t.row_count_estimate ?? 0))
  );
  let wastedPct = $derived(
    stats && stats.page_count > 0
      ? Math.round((stats.freelist_pages / stats.page_count) * 100)
      : 0
  );
</script>

<div class="flex-1 flex flex-col h-full bg-[#121214] text-xs select-none overflow-hidden">
  <!-- Header -->
  <div class="h-10 border-b border-[#27272a] bg-[#18181b] flex items-center justify-between px-4">
    <div class="flex items-center gap-2 font-semibold text-zinc-200">
      <Gauge class="w-4 h-4 text-sky-400" />
      <span>{t('dash.title')}</span>
      {#if stats}
        <span class="text-[10px] text-zinc-500 font-mono">{stats.version}</span>
      {/if}
    </div>
    <button
      class="flex items-center gap-1 px-2.5 py-1 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-200 transition-colors"
      onclick={load}
    >
      <RefreshCw class="w-3.5 h-3.5" /><span>{t('common.refresh')}</span>
    </button>
  </div>

  <div class="flex-1 overflow-auto p-4 space-y-4">
    {#if loading}
      <div class="flex items-center justify-center h-40 text-zinc-500 gap-2">
        <div class="w-4 h-4 border-2 border-sky-500 border-t-transparent rounded-full animate-spin"></div>
        <span>{t('dash.loading')}</span>
      </div>
    {:else if loadError}
      <div class="p-4 bg-rose-950/40 border-l-4 border-rose-500 text-rose-300 font-mono">{loadError}</div>
    {:else if stats}
      <!-- Primary stat cards -->
      <div class="grid grid-cols-4 gap-3">
        <div class="rounded-lg border border-[#27272a] bg-[#18181b] p-3">
          <div class="flex items-center gap-1.5 text-[10px] uppercase tracking-wider text-zinc-500">
            <HardDrive class="w-3 h-3" /> {t('dash.db_size')}
          </div>
          <div class="text-xl font-bold text-zinc-100 font-mono mt-1">{fmtBytes(stats.file_size_bytes)}</div>
          <div class="text-[10px] text-zinc-500 font-mono mt-0.5">
            {t('dash.page_detail', { pages: stats.page_count, size: stats.page_size })}
          </div>
        </div>
        <div class="rounded-lg border border-[#27272a] bg-[#18181b] p-3">
          <div class="flex items-center gap-1.5 text-[10px] uppercase tracking-wider text-zinc-500">
            <Rows3 class="w-3 h-3" /> {t('dash.total_rows')}
          </div>
          <div class="text-xl font-bold text-zinc-100 font-mono mt-1">{stats.total_rows.toLocaleString()}</div>
          <div class="text-[10px] text-zinc-500 font-mono mt-0.5">{t('dash.across_tables', { count: stats.table_count })}</div>
        </div>
        <div class="rounded-lg border border-[#27272a] bg-[#18181b] p-3">
          <div class="flex items-center gap-1.5 text-[10px] uppercase tracking-wider text-zinc-500">
            <Layers class="w-3 h-3" /> {t('dash.free_space')}
          </div>
          <div class={`text-xl font-bold font-mono mt-1 ${wastedPct > 15 ? 'text-amber-400' : 'text-zinc-100'}`}>
            {fmtBytes(stats.freelist_pages * stats.page_size)}
          </div>
          <div class="text-[10px] text-zinc-500 font-mono mt-0.5">
            {t('dash.fragmented', { pct: wastedPct })} {wastedPct > 15 ? t('dash.consider_vacuum') : ''}
          </div>
        </div>
        <div class="rounded-lg border border-[#27272a] bg-[#18181b] p-3">
          <div class="flex items-center gap-1.5 text-[10px] uppercase tracking-wider text-zinc-500">
            <Clock class="w-3 h-3" /> {t('dash.queries')}
          </div>
          <div class="text-xl font-bold text-zinc-100 font-mono mt-1">{audit?.total ?? 0}</div>
          <div class="text-[10px] font-mono mt-0.5 {audit && audit.error_count > 0 ? 'text-rose-400' : 'text-zinc-500'}">
            {t('dash.errors_avg', { errors: audit?.error_count ?? 0, avg: audit?.avg_duration_ms.toFixed(1) ?? '0' })}
          </div>
        </div>
      </div>

      <div class="grid grid-cols-2 gap-3">
        <!-- Storage & Engine -->
        <div class="rounded-lg border border-[#27272a] bg-[#18181b] overflow-hidden">
          <div class="p-3 border-b border-[#27272a] font-semibold text-zinc-300 flex items-center gap-2">
            <Database class="w-3.5 h-3.5 text-sky-400" /> {t('dash.engine')}
          </div>
          <table class="w-full font-mono text-[11px]">
            <tbody>
              <tr class="border-b border-[#27272a]/50">
                <td class="px-3 py-2 text-zinc-500 w-40">{t('dash.row_engine')}</td>
                <td class="px-3 py-2 text-zinc-200">{stats.version}</td>
              </tr>
              <tr class="border-b border-[#27272a]/50">
                <td class="px-3 py-2 text-zinc-500">{t('dash.row_file')}</td>
                <td class="px-3 py-2 text-zinc-200 truncate select-text" title={stats.file_path ?? ''}>
                  {stats.file_path ?? t('dash.memory_db')}
                </td>
              </tr>
              <tr class="border-b border-[#27272a]/50">
                <td class="px-3 py-2 text-zinc-500">{t('dash.row_journal')}</td>
                <td class="px-3 py-2 text-zinc-200 uppercase">{stats.journal_mode || 'default'}</td>
              </tr>
              <tr class="border-b border-[#27272a]/50">
                <td class="px-3 py-2 text-zinc-500">{t('dash.row_encoding')}</td>
                <td class="px-3 py-2 text-zinc-200">{stats.encoding || 'UTF-8'}</td>
              </tr>
              <tr>
                <td class="px-3 py-2 text-zinc-500">{t('dash.row_conn')}</td>
                <td class="px-3 py-2 text-zinc-200">{workspace.activeConnection?.name ?? '-'}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- Schema Objects -->
        <div class="rounded-lg border border-[#27272a] bg-[#18181b] overflow-hidden">
          <div class="p-3 border-b border-[#27272a] font-semibold text-zinc-300 flex items-center gap-2">
            <FileText class="w-3.5 h-3.5 text-amber-400" /> {t('dash.objects')}
          </div>
          <div class="grid grid-cols-2 gap-2 p-3">
            <button
              class="flex items-center gap-2.5 p-2.5 rounded-lg bg-[#121214] border border-[#27272a] hover:border-sky-500/40 transition-colors text-left"
              onclick={() => workspace.openErdTab()}
            >
              <Table class="w-4 h-4 text-sky-400" />
              <div>
                <div class="font-mono font-bold text-zinc-100">{stats.table_count}</div>
                <div class="text-[10px] text-zinc-500">{t('dash.tables')}</div>
              </div>
            </button>
            <div class="flex items-center gap-2.5 p-2.5 rounded-lg bg-[#121214] border border-[#27272a]">
              <Eye class="w-4 h-4 text-indigo-400" />
              <div>
                <div class="font-mono font-bold text-zinc-100">{stats.view_count}</div>
                <div class="text-[10px] text-zinc-500">{t('dash.views')}</div>
              </div>
            </div>
            <div class="flex items-center gap-2.5 p-2.5 rounded-lg bg-[#121214] border border-[#27272a]">
              <KeyRound class="w-4 h-4 text-amber-400" />
              <div>
                <div class="font-mono font-bold text-zinc-100">{stats.index_count}</div>
                <div class="text-[10px] text-zinc-500">{t('dash.indexes')}</div>
              </div>
            </div>
            <div class="flex items-center gap-2.5 p-2.5 rounded-lg bg-[#121214] border border-[#27272a]">
              <Zap class="w-4 h-4 text-violet-400" />
              <div>
                <div class="font-mono font-bold text-zinc-100">{stats.trigger_count}</div>
                <div class="text-[10px] text-zinc-500">{t('dash.triggers')}</div>
              </div>
            </div>
          </div>
          {#if audit}
            <div class="px-3 pb-3">
              <button
                class="w-full flex items-center justify-center gap-1.5 py-1.5 rounded bg-violet-500/15 hover:bg-violet-500/25 text-violet-300 border border-violet-500/30 text-[11px] transition-colors"
                onclick={() => workspace.openAuditTab()}
              >
                <ShieldCheck class="w-3.5 h-3.5" /> {t('dash.open_audit')}
              </button>
            </div>
          {/if}
        </div>
      </div>

      <!-- Per-table row distribution -->
      <div class="rounded-lg border border-[#27272a] bg-[#18181b] overflow-hidden">
        <div class="p-3 border-b border-[#27272a] font-semibold text-zinc-300">
          {t('dash.rows_per_table')}
        </div>
        <div class="p-3 space-y-1.5">
          {#each workspace.tables.filter((tbl) => tbl.table_type === 'table') as tbl}
            {@const n = tbl.row_count_estimate ?? 0}
            {@const pct = Math.round((n / maxTableRows) * 100)}
            <button
              class="w-full flex items-center gap-2 group"
              onclick={() => workspace.openTableTab(tbl.name)}
              title={t('dash.open_table', { name: tbl.name })}
            >
              <span class="w-32 truncate text-right text-zinc-400 group-hover:text-zinc-200 font-mono text-[10px]">{tbl.name}</span>
              <div class="flex-1 h-4 rounded bg-[#121214] overflow-hidden">
                <div
                  class="h-full rounded bg-gradient-to-r from-sky-600/70 to-indigo-500/70 group-hover:from-sky-500 group-hover:to-indigo-400 transition-all"
                  style={`width: ${Math.max(pct, n > 0 ? 2 : 0)}%`}
                ></div>
              </div>
              <span class="w-16 text-zinc-400 font-mono text-[10px]">{n.toLocaleString()}</span>
            </button>
          {/each}
          {#if workspace.tables.filter((tbl) => tbl.table_type === 'table').length === 0}
            <div class="text-zinc-500 italic p-2">{t('dash.no_tables')}</div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>
