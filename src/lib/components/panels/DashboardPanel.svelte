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

<div class="flex-1 flex flex-col h-full bg-app text-xs select-none overflow-hidden">
  <!-- Header -->
  <div class="h-10 border-b border-edge bg-panel flex items-center justify-between px-4">
    <div class="flex items-center gap-2 font-semibold text-fg">
      <Gauge class="w-4 h-4 text-accent" />
      <span>{t('dash.title')}</span>
      {#if stats}
        <span class="text-[10px] text-faint font-mono">{stats.version}</span>
      {/if}
    </div>
    <button
      class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-elevated hover:bg-hover border border-edge text-fg transition-colors"
      onclick={load}
    >
      <RefreshCw class="w-3.5 h-3.5" /><span>{t('common.refresh')}</span>
    </button>
  </div>

  <div class="flex-1 overflow-auto p-4 space-y-4">
    {#if loading}
      <!-- Layout-matched skeletal loader -->
      <div class="space-y-4 animate-pulse" role="status" aria-label={t('dash.loading')}>
        <div class="grid grid-cols-4 gap-3">
          {#each Array(4) as _}
            <div class="rounded-lg border border-edge bg-panel p-3 space-y-2">
              <div class="h-2.5 w-20 rounded bg-elevated"></div>
              <div class="h-6 w-24 rounded bg-elevated"></div>
              <div class="h-2 w-28 rounded bg-subtle"></div>
            </div>
          {/each}
        </div>
        <div class="grid grid-cols-2 gap-3">
          <div class="rounded-lg border border-edge bg-panel p-3 space-y-3">
            <div class="h-3 w-32 rounded bg-elevated"></div>
            {#each Array(5) as _}
              <div class="flex justify-between gap-4">
                <div class="h-2.5 w-24 rounded bg-subtle"></div>
                <div class="h-2.5 w-40 rounded bg-elevated"></div>
              </div>
            {/each}
          </div>
          <div class="rounded-lg border border-edge bg-panel p-3 space-y-3">
            <div class="h-3 w-32 rounded bg-elevated"></div>
            <div class="grid grid-cols-2 gap-2">
              {#each Array(4) as _}
                <div class="h-12 rounded-lg bg-app border border-edge"></div>
              {/each}
            </div>
          </div>
        </div>
      </div>
    {:else if loadError}
      <div class="p-4 bg-err-bg border-l-2 border-err text-err font-mono">{loadError}</div>
    {:else if stats}
      <!-- Primary stat cards -->
      <div class="grid grid-cols-4 gap-3">
        <div class="rounded-lg border border-edge bg-panel p-3">
          <div class="flex items-center gap-1.5 text-[10px] uppercase tracking-wider text-faint">
            <HardDrive class="w-3 h-3" /> {t('dash.db_size')}
          </div>
          <div class="text-xl font-bold text-fg font-mono mt-1">{fmtBytes(stats.file_size_bytes)}</div>
          <div class="text-[10px] text-faint font-mono mt-0.5">
            {t('dash.page_detail', { pages: stats.page_count, size: stats.page_size })}
          </div>
        </div>
        <div class="rounded-lg border border-edge bg-panel p-3">
          <div class="flex items-center gap-1.5 text-[10px] uppercase tracking-wider text-faint">
            <Rows3 class="w-3 h-3" /> {t('dash.total_rows')}
          </div>
          <div class="text-xl font-bold text-fg font-mono mt-1">{stats.total_rows.toLocaleString()}</div>
          <div class="text-[10px] text-faint font-mono mt-0.5">{t('dash.across_tables', { count: stats.table_count })}</div>
        </div>
        <div class="rounded-lg border border-edge bg-panel p-3">
          <div class="flex items-center gap-1.5 text-[10px] uppercase tracking-wider text-faint">
            <Layers class="w-3 h-3" /> {t('dash.free_space')}
          </div>
          <div class={`text-xl font-bold font-mono mt-1 ${wastedPct > 15 ? 'text-warn' : 'text-fg'}`}>
            {fmtBytes(stats.freelist_pages * stats.page_size)}
          </div>
          <div class="text-[10px] text-faint font-mono mt-0.5">
            {t('dash.fragmented', { pct: wastedPct })} {wastedPct > 15 ? t('dash.consider_vacuum') : ''}
          </div>
        </div>
        <div class="rounded-lg border border-edge bg-panel p-3">
          <div class="flex items-center gap-1.5 text-[10px] uppercase tracking-wider text-faint">
            <Clock class="w-3 h-3" /> {t('dash.queries')}
          </div>
          <div class="text-xl font-bold text-fg font-mono mt-1">{audit?.total ?? 0}</div>
          <div class="text-[10px] font-mono mt-0.5 {audit && audit.error_count > 0 ? 'text-err' : 'text-faint'}">
            {t('dash.errors_avg', { errors: audit?.error_count ?? 0, avg: audit?.avg_duration_ms.toFixed(1) ?? '0' })}
          </div>
        </div>
      </div>

      <div class="grid grid-cols-2 gap-3">
        <!-- Storage & Engine -->
        <div class="rounded-lg border border-edge bg-panel overflow-hidden">
          <div class="p-3 border-b border-edge font-semibold text-fg2 flex items-center gap-2">
            <Database class="w-3.5 h-3.5 text-accent" /> {t('dash.engine')}
          </div>
          <table class="w-full font-mono text-[11px]">
            <tbody>
              <tr class="border-b border-edge/50">
                <td class="px-3 py-2 text-faint w-40">{t('dash.row_engine')}</td>
                <td class="px-3 py-2 text-fg">{stats.version}</td>
              </tr>
              <tr class="border-b border-edge/50">
                <td class="px-3 py-2 text-faint">{t('dash.row_file')}</td>
                <td class="px-3 py-2 text-fg truncate select-text" title={stats.file_path ?? ''}>
                  {stats.file_path ?? t('dash.memory_db')}
                </td>
              </tr>
              <tr class="border-b border-edge/50">
                <td class="px-3 py-2 text-faint">{t('dash.row_journal')}</td>
                <td class="px-3 py-2 text-fg uppercase">{stats.journal_mode || 'default'}</td>
              </tr>
              <tr class="border-b border-edge/50">
                <td class="px-3 py-2 text-faint">{t('dash.row_encoding')}</td>
                <td class="px-3 py-2 text-fg">{stats.encoding || 'UTF-8'}</td>
              </tr>
              <tr>
                <td class="px-3 py-2 text-faint">{t('dash.row_conn')}</td>
                <td class="px-3 py-2 text-fg">{workspace.activeConnection?.name ?? '-'}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- Schema Objects -->
        <div class="rounded-lg border border-edge bg-panel overflow-hidden">
          <div class="p-3 border-b border-edge font-semibold text-fg2 flex items-center gap-2">
            <FileText class="w-3.5 h-3.5 text-accent" /> {t('dash.objects')}
          </div>
          <div class="grid grid-cols-2 gap-2 p-3">
            <button
              class="flex items-center gap-2.5 p-2.5 rounded-lg bg-app border border-edge hover:border-accent-edge transition-colors text-left"
              onclick={() => workspace.openErdTab()}
            >
              <Table class="w-4 h-4 text-accent" />
              <div>
                <div class="font-mono font-bold text-fg">{stats.table_count}</div>
                <div class="text-[10px] text-faint">{t('dash.tables')}</div>
              </div>
            </button>
            <div class="flex items-center gap-2.5 p-2.5 rounded-lg bg-app border border-edge">
              <Eye class="w-4 h-4 text-muted" />
              <div>
                <div class="font-mono font-bold text-fg">{stats.view_count}</div>
                <div class="text-[10px] text-faint">{t('dash.views')}</div>
              </div>
            </div>
            <div class="flex items-center gap-2.5 p-2.5 rounded-lg bg-app border border-edge">
              <KeyRound class="w-4 h-4 text-muted" />
              <div>
                <div class="font-mono font-bold text-fg">{stats.index_count}</div>
                <div class="text-[10px] text-faint">{t('dash.indexes')}</div>
              </div>
            </div>
            <div class="flex items-center gap-2.5 p-2.5 rounded-lg bg-app border border-edge">
              <Zap class="w-4 h-4 text-muted" />
              <div>
                <div class="font-mono font-bold text-fg">{stats.trigger_count}</div>
                <div class="text-[10px] text-faint">{t('dash.triggers')}</div>
              </div>
            </div>
          </div>
          {#if audit}
            <div class="px-3 pb-3">
              <button
                class="w-full flex items-center justify-center gap-1.5 py-1.5 rounded-md bg-accent-soft hover:opacity-90 text-accent border border-accent-edge text-[11px] font-medium transition-colors"
                onclick={() => workspace.openAuditTab()}
              >
                <ShieldCheck class="w-3.5 h-3.5" /> {t('dash.open_audit')}
              </button>
            </div>
          {/if}
        </div>
      </div>

      <!-- Per-table row distribution -->
      <div class="rounded-lg border border-edge bg-panel overflow-hidden">
        <div class="p-3 border-b border-edge font-semibold text-fg2">
          {t('dash.rows_per_table')}
        </div>
        <div class="p-3 space-y-1.5">
          {#each workspace.tables.filter((tbl) => tbl.table_type === 'table') as tbl}
            {@const n = tbl.row_count_estimate ?? 0}
            {@const pct = Math.round((n / maxTableRows) * 100)}
            <button
              class="w-full flex items-center gap-3 py-1 px-1.5 rounded hover:bg-hover group transition-colors"
              onclick={() => workspace.openTableTab(tbl.name)}
              title={t('dash.open_table', { name: tbl.name })}
            >
              <span class="w-36 truncate text-left text-fg2 group-hover:text-fg font-mono text-[11px]">{tbl.name}</span>
              <div class="flex-1 flex items-center">
                <div
                  class="h-2 rounded-sm bg-accent/75 group-hover:bg-accent transition-all"
                  style={`width: ${Math.max(pct, n > 0 ? 2 : 0)}%`}
                ></div>
              </div>
              <span class="w-20 text-right text-muted font-mono text-[11px]">{n.toLocaleString()}</span>
            </button>
          {/each}
          {#if workspace.tables.filter((tbl) => tbl.table_type === 'table').length === 0}
            <div class="text-faint p-4 text-center">{t('dash.no_tables')}</div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>
