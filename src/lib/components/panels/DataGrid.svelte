<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import type { QueryResultData, TableColumnMetadata } from '$lib/types';
  import {
    Plus,
    Trash2,
    Save,
    RotateCcw,
    Code,
    Filter,
    ChevronLeft,
    ChevronRight,
    ChevronsLeft,
    ChevronsRight,
    Search,
    SlidersHorizontal,
    Copy,
    Maximize2,
    X,
    Edit3,
    Download,
    FolderOpen,
  } from 'lucide-svelte';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { buildInsertSql } from '$lib/utils/sql';
  import { t } from '$lib/i18n/i18n.svelte';
  import { toasts } from '$lib/stores/toast.svelte';
  import { confirmDialog } from '$lib/stores/confirm.svelte';

  let { tableName }: { tableName: string } = $props();

  let loading = $state(false);
  let tableData = $state<QueryResultData | null>(null);
  let schema = $state<TableColumnMetadata[]>([]);
  let primaryKeys = $derived(schema.filter((c) => c.is_primary_key).map((c) => c.name));

  // Pagination & Filtering
  let limit = $state(100);
  let offset = $state(0);
  let filterText = $state('');
  let sortBy = $state<string | null>(null);
  let sortDir = $state<'ASC' | 'DESC'>('ASC');

  // Editing state
  let editingCell = $state<{ rowIndex: number; colName: string } | null>(null);
  let editValue = $state('');
  let selectedRows = $state<Set<number>>(new Set());

  // Row Detail Drawer (Spacebar)
  let detailDrawerRow = $state<any[] | null>(null);
  let copiedKind = $state<string | null>(null);

  function copyRowAs(kind: 'insert' | 'json') {
    if (!detailDrawerRow || !tableData) return;
    const obj: Record<string, unknown> = {};
    tableData.columns.forEach((c, i) => (obj[c] = detailDrawerRow![i]));
    const text = kind === 'insert' ? buildInsertSql(tableName, obj) : JSON.stringify(obj, null, 2);
    navigator.clipboard.writeText(text);
    copiedKind = kind;
    setTimeout(() => (copiedKind = null), 1500);
  }

  // Bulk Edit Modal (HeidiSQL Feature)
  let isBulkEditOpen = $state(false);
  let bulkEditColumn = $state('');
  let bulkEditValue = $state('');

  // Export menu
  let isExportMenuOpen = $state(false);

  async function exportTable(format: 'csv' | 'json' | 'sql' | 'xlsx') {
    if (!workspace.activeConnection) return;
    isExportMenuOpen = false;
    try {
      // Export respects the currently active WHERE filter.
      const res = await api.exportTable(
        workspace.activeConnection.id,
        tableName,
        format,
        filterText || undefined
      );
      toasts.success(t('grid.exported_msg', { count: res.row_count, path: res.file_path }));
    } catch (err: any) {
      toasts.error(t('grid.export_failed') + err);
    }
  }

  async function openExportsFolder() {
    try {
      await openPath(await api.getDataDir('exports'));
    } catch (err: any) {
      toasts.error(t('grid.open_exports_failed') + err);
    }
  }

  // Staged changes for this table
  let staged = $derived(workspace.getStagedForTable(tableName));
  let totalStaged = $derived(workspace.getTotalStagedCount(tableName));

  let totalPages = $derived(Math.max(1, Math.ceil((tableData?.total_rows ?? 0) / limit)));
  let currentPage = $derived(Math.min(totalPages, Math.floor(offset / limit) + 1));

  function goToPage(page: number) {
    offset = Math.min(Math.max(page - 1, 0), totalPages - 1) * limit;
    loadData();
  }

  function applyFilter() {
    offset = 0;
    loadData();
  }

  function setLimit(n: number) {
    limit = n;
    offset = 0;
    loadData();
  }

  async function loadData() {
    if (!workspace.activeConnection) return;
    loading = true;
    try {
      schema = await api.getTableSchema(workspace.activeConnection.id, tableName);
      tableData = await api.getTableData(
        workspace.activeConnection.id,
        tableName,
        limit,
        offset,
        filterText || undefined,
        sortBy || undefined,
        sortDir
      );
    } catch (err: any) {
      toasts.error(t('grid.load_failed') + err);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    // Reload when tableName changes
    if (tableName) {
      loadData();
    }
  });

  function getPkValues(row: any[]): Record<string, any> {
    const pks: Record<string, any> = {};
    if (primaryKeys.length > 0 && tableData) {
      for (const pk of primaryKeys) {
        const idx = tableData.columns.indexOf(pk);
        if (idx !== -1) {
          pks[pk] = row[idx];
        }
      }
    } else if (tableData && tableData.columns.length > 0) {
      // Fallback: use first column as identifier
      pks[tableData.columns[0]] = row[0];
    }
    return pks;
  }

  function startEdit(rowIndex: number, colName: string, currentValue: any) {
    editingCell = { rowIndex, colName };
    editValue = currentValue === null ? '' : String(currentValue);
  }

  function saveCellEdit(row: any[]) {
    if (!editingCell || !tableData) return;
    const { colName } = editingCell;
    const pkValues = getPkValues(row);

    // Save into staged updates
    workspace.stageUpdate(tableName, pkValues, { [colName]: editValue });
    editingCell = null;
  }

  function addNewRow() {
    if (!tableData) return;
    const newRowObj: Record<string, any> = {};
    for (const col of tableData.columns) {
      newRowObj[col] = '';
    }
    workspace.stageInsert(tableName, newRowObj);
  }

  async function markDeleteSelected() {
    if (!tableData || selectedRows.size === 0) return;
    const ok = await confirmDialog.ask({
      title: t('confirm.delete_rows_title'),
      message: t('confirm.delete_rows_msg', { count: selectedRows.size }),
      confirmLabel: t('confirm.delete'),
      danger: true,
    });
    if (!ok) return;
    for (const rowIdx of selectedRows) {
      const row = tableData.rows[rowIdx];
      if (row) {
        const pkValues = getPkValues(row);
        workspace.stageDelete(tableName, pkValues);
      }
    }
    selectedRows.clear();
  }

  async function requestDiscard() {
    const ok = await confirmDialog.ask({
      title: t('confirm.discard_changes_title'),
      message: t('confirm.discard_changes_msg', { count: totalStaged, table: tableName }),
      confirmLabel: t('confirm.discard'),
      danger: true,
    });
    if (ok) workspace.discardChanges(tableName);
  }

  function applyBulkEdit() {
    if (!tableData || !bulkEditColumn) return;
    for (const rowIdx of selectedRows) {
      const row = tableData.rows[rowIdx];
      if (row) {
        const pkValues = getPkValues(row);
        workspace.stageUpdate(tableName, pkValues, { [bulkEditColumn]: bulkEditValue });
      }
    }
    isBulkEditOpen = false;
    selectedRows.clear();
  }

  function toggleSort(colName: string) {
    if (sortBy === colName) {
      if (sortDir === 'ASC') {
        sortDir = 'DESC';
      } else {
        sortBy = null;
        sortDir = 'ASC';
      }
    } else {
      sortBy = colName;
      sortDir = 'ASC';
    }
    offset = 0;
    loadData();
  }

  // Keyboard navigation & spacebar for detail drawer
  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === ' ' && selectedRows.size === 1 && !editingCell) {
      e.preventDefault();
      const rowIdx = Array.from(selectedRows)[0];
      if (tableData && tableData.rows[rowIdx]) {
        detailDrawerRow = detailDrawerRow ? null : tableData.rows[rowIdx];
      }
    } else if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      if (totalStaged > 0) {
        workspace.previewStaged(tableName, primaryKeys);
      }
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="flex-1 flex flex-col h-full bg-[#121214] text-xs select-none overflow-hidden relative">
  <!-- Toolbar: Filter bar & Actions -->
  <div class="h-10 border-b border-[#27272a] bg-[#18181b] flex items-center justify-between px-3 gap-2">
    <!-- Filter Builder -->
    <div class="flex items-center gap-2 flex-1 max-w-lg">
      <Filter class="w-3.5 h-3.5 text-zinc-400" />
      <input
        type="text"
        placeholder={t('grid.filter_placeholder')}
        bind:value={filterText}
        onkeydown={(e) => e.key === 'Enter' && applyFilter()}
        class="w-full bg-[#141416] border border-[#2e2e33] rounded px-2.5 py-1 text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-sky-500 transition-colors font-mono text-[11px]"
      />
      <button
        class="px-2.5 py-1 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-300 font-medium transition-colors"
        onclick={applyFilter}
      >
        {t('common.apply')}
      </button>
    </div>

    <!-- Actions: Add Row, Delete Row, Bulk Edit (HeidiSQL), Discard, Commit -->
    <div class="flex items-center gap-1.5">
      <button
        class="flex items-center gap-1 px-2 py-1 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-200 transition-colors"
        onclick={addNewRow}
        title={t('grid.add_row_hint')}
      >
        <Plus class="w-3.5 h-3.5 text-emerald-400" />
        <span>{t('grid.add_row')}</span>
      </button>

      <!-- Export dropdown -->
      <div class="relative">
        <button
          class="flex items-center gap-1 px-2 py-1 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-200 transition-colors"
          onclick={() => (isExportMenuOpen = !isExportMenuOpen)}
          title={t('grid.export_hint')}
        >
          <Download class="w-3.5 h-3.5 text-sky-400" />
          <span>{t('common.export')}</span>
        </button>
        {#if isExportMenuOpen}
          <div class="absolute right-0 top-8 w-40 rounded-lg border border-[#27272a] bg-[#1c1c20] shadow-2xl z-40 p-1 space-y-0.5">
            {#each [
              ['csv', 'CSV (.csv)'],
              ['xlsx', 'Excel (.xlsx)'],
              ['json', 'JSON (.json)'],
              ['sql', 'SQL Dump (.sql)'],
            ] as [fmt, label]}
              <button
                class="w-full text-left px-2.5 py-1.5 rounded text-zinc-200 hover:bg-[#27272a] transition-colors"
                onclick={() => exportTable(fmt as 'csv' | 'json' | 'sql' | 'xlsx')}
              >
                {label}
              </button>
            {/each}
            {#if filterText}
              <div class="px-2.5 pb-1 text-[9px] text-zinc-500 font-mono">{t('grid.filtered_note', { filter: filterText })}</div>
            {/if}
            <div class="border-t border-[#27272a] my-1"></div>
            <button
              class="w-full text-left px-2.5 py-1.5 rounded text-zinc-400 hover:bg-[#27272a] flex items-center gap-1.5"
              onclick={openExportsFolder}
            >
              <FolderOpen class="w-3 h-3" /> {t('grid.open_exports')}
            </button>
          </div>
        {/if}
      </div>

      {#if selectedRows.size > 0}
        <!-- Bulk Edit Button (HeidiSQL) -->
        <button
          class="flex items-center gap-1 px-2 py-1 rounded bg-amber-500/20 hover:bg-amber-500/30 text-amber-300 border border-amber-500/40 transition-colors"
          onclick={() => {
            if (tableData && tableData.columns.length > 0) {
              bulkEditColumn = tableData.columns[0];
            }
            isBulkEditOpen = true;
          }}
          title={t('grid.bulk_edit_hint')}
        >
          <Edit3 class="w-3.5 h-3.5" />
          <span>{t('grid.bulk_edit', { count: selectedRows.size })}</span>
        </button>

        <button
          class="flex items-center gap-1 px-2 py-1 rounded bg-rose-500/20 hover:bg-rose-500/30 text-rose-300 border border-rose-500/40 transition-colors"
          onclick={markDeleteSelected}
          title={t('grid.delete_hint')}
        >
          <Trash2 class="w-3.5 h-3.5" />
          <span>{t('grid.delete_rows', { count: selectedRows.size })}</span>
        </button>
      {/if}

    </div>
  </div>

  <!-- Data Grid Virtualized Area -->
  <div class="flex-1 overflow-auto bg-[#121214] relative">
    {#if loading && tableData}
      <!-- Page-change progress bar: keeps current rows visible while fetching -->
      <div class="absolute top-0 left-0 right-0 h-0.5 z-20 overflow-hidden">
        <div class="h-full w-1/3 bg-sky-500 animate-[slide_1s_ease-in-out_infinite]" style="animation-name: datagrid-slide"></div>
      </div>
    {/if}
    {#if loading && !tableData}
      <!-- Loading skeleton: shimmer rows shaped like the grid -->
      {@const skelCols = Math.max(4, schema.length || 6)}
      <div class="w-full font-mono text-[11px] animate-pulse" role="status" aria-label={t('grid.loading')}>
        <div class="flex border-b border-[#27272a] bg-[#18181b]">
          <div class="w-10 px-2 py-2"></div>
          {#each Array(skelCols) as _}
            <div class="flex-1 px-3 py-2"><div class="h-2.5 w-2/3 rounded bg-[#27272a]"></div></div>
          {/each}
        </div>
        {#each Array(18) as _, r}
          <div class="flex border-b border-[#1f1f23]">
            <div class="w-10 px-2 py-2"></div>
            {#each Array(skelCols) as _, c}
              <div class="flex-1 px-3 py-2">
                <div
                  class={`h-2.5 rounded ${r % 2 === 0 ? 'bg-[#1a1a1e]' : 'bg-[#1e1e22]'}`}
                  style={`width: ${45 + ((r * 7 + c * 13) % 45)}%`}
                ></div>
              </div>
            {/each}
          </div>
        {/each}
      </div>
    {:else if !tableData || tableData.columns.length === 0}
      <div class="p-8 text-center text-zinc-500">{t('grid.empty')}</div>
    {:else}
      <table class="w-full text-left border-collapse font-mono text-[11px]">
        <!-- Table Header -->
        <thead class="sticky top-0 bg-[#18181b] border-b border-[#27272a] text-zinc-400 z-10">
          <tr>
            <th class="w-10 px-2 py-1.5 border-r border-[#27272a] text-center">
              <input
                type="checkbox"
                checked={selectedRows.size === tableData.rows.length && tableData.rows.length > 0}
                onchange={(e) => {
                  if ((e.target as HTMLInputElement).checked) {
                    selectedRows = new Set(tableData!.rows.map((_, i) => i));
                  } else {
                    selectedRows.clear();
                  }
                }}
                class="rounded bg-[#27272a] border-zinc-700"
              />
            </th>
            {#each tableData.columns as col (col)}
              {@const isPk = primaryKeys.includes(col)}
              <th
                class="px-3 py-1.5 border-r border-[#27272a] hover:bg-[#222226] cursor-pointer select-none truncate"
                onclick={() => toggleSort(col)}
              >
                <div class="flex items-center justify-between gap-1">
                  <div class="flex items-center gap-1 truncate">
                    {#if isPk}
                      <span class="text-amber-400 font-bold" title={t('grid.pk')}>🔑</span>
                    {/if}
                    <span class={isPk ? 'text-amber-200 font-semibold' : 'text-zinc-200 font-medium'}>
                      {col}
                    </span>
                  </div>
                  {#if sortBy === col}
                    <span class="text-sky-400 text-[10px]">{sortDir === 'ASC' ? '▲' : '▼'}</span>
                  {/if}
                </div>
              </th>
            {/each}
          </tr>
        </thead>

        <!-- Table Body -->
        <tbody>
          <!-- 1. Staged Inserts (Green Rows) -->
          {#each staged.inserts as newRow, i}
            <tr class="bg-emerald-950/40 text-emerald-300 border-b border-emerald-900/40 font-medium">
              <td class="px-2 py-1 border-r border-[#27272a] text-center text-[10px]">
                <span class="text-emerald-400 font-bold">+</span>
              </td>
              {#each tableData.columns as col}
                <td class="px-3 py-1 border-r border-emerald-900/30 truncate">
                  <input
                    type="text"
                    bind:value={newRow[col]}
                    placeholder={t('grid.auto')}
                    class="w-full bg-transparent text-emerald-200 focus:outline-none"
                  />
                </td>
              {/each}
            </tr>
          {/each}

          <!-- 2. Existing Database Rows -->
          {#each tableData.rows as row, rowIdx (rowIdx)}
            {@const pkVals = getPkValues(row)}
            {@const pkStr = JSON.stringify(pkVals)}
            {@const isUpdated = staged.updates.some((u) => JSON.stringify(u.pk) === pkStr)}
            {@const isDeleted = staged.deletes.some((d) => JSON.stringify(d) === pkStr)}
            {@const isSelected = selectedRows.has(rowIdx)}

            <tr
              class={`border-b border-[#1f1f23] transition-colors ${
                isDeleted
                  ? 'bg-rose-950/40 text-rose-400 line-through'
                  : isUpdated
                  ? 'bg-amber-950/40 text-amber-300'
                  : isSelected
                  ? 'bg-sky-950/40 text-zinc-100'
                  : rowIdx % 2 === 0
                  ? 'bg-[#121214] hover:bg-[#1a1a1e]'
                  : 'bg-[#151518] hover:bg-[#1a1a1e]'
              }`}
              onclick={() => {
                if (selectedRows.has(rowIdx)) {
                  selectedRows.delete(rowIdx);
                } else {
                  selectedRows.add(rowIdx);
                }
              }}
            >
              <!-- Row Selection Checkbox -->
              <td class="px-2 py-1.5 border-r border-[#27272a] text-center" onclick={(e) => e.stopPropagation()}>
                <input
                  type="checkbox"
                  checked={isSelected}
                  onchange={(e) => {
                    if ((e.target as HTMLInputElement).checked) {
                      selectedRows.add(rowIdx);
                    } else {
                      selectedRows.delete(rowIdx);
                    }
                  }}
                  class="rounded bg-[#27272a] border-zinc-700"
                />
              </td>

              <!-- Cells -->
              {#each tableData.columns as col, colIdx (col)}
                {@const cellVal = row[colIdx]}
                {@const updateEntry = staged.updates.find((u) => JSON.stringify(u.pk) === pkStr)}
                {@const modifiedVal = updateEntry?.fields[col]}
                {@const isCellModified = modifiedVal !== undefined}
                {@const displayVal = isCellModified ? modifiedVal : cellVal}
                {@const isEditing = editingCell?.rowIndex === rowIdx && editingCell?.colName === col}

                <td
                  class={`px-3 py-1.5 border-r border-[#27272a] truncate max-w-[240px] cursor-cell ${
                    isCellModified ? 'bg-amber-500/20 text-amber-200 font-semibold' : ''
                  }`}
                  ondblclick={(e) => {
                    e.stopPropagation();
                    startEdit(rowIdx, col, displayVal);
                  }}
                >
                  {#if isEditing}
                    <input
                      type="text"
                      bind:value={editValue}
                      onkeydown={(e) => {
                        if (e.key === 'Enter') saveCellEdit(row);
                        if (e.key === 'Escape') editingCell = null;
                      }}
                      onblur={() => saveCellEdit(row)}
                      autofocus
                      class="w-full bg-[#18181b] border border-sky-500 text-zinc-100 px-1 py-0.5 rounded focus:outline-none text-[11px]"
                    />
                  {:else if displayVal === null}
                    <span class="text-zinc-600 italic">NULL</span>
                  {:else if typeof displayVal === 'boolean'}
                    <span class={displayVal ? 'text-emerald-400' : 'text-rose-400'}>
                      {displayVal ? 'true' : 'false'}
                    </span>
                  {:else}
                    <span class="truncate">{displayVal}</span>
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <!-- Status bar: pagination -->
  {#if tableData}
    <div class="h-8 shrink-0 border-t border-[#27272a] bg-[#18181b] flex items-center justify-between px-3 text-[10px] font-mono text-zinc-500">
      <div class="flex items-center gap-3">
        <span class="text-zinc-400">{t('common.rows', { count: tableData.total_rows.toLocaleString() })}</span>
        {#if filterText}
          <span class="px-1.5 py-0.5 rounded bg-amber-500/10 border border-amber-500/30 text-amber-400 truncate max-w-[220px]" title={`WHERE ${filterText}`}>
            {t('grid.filtered')}: {filterText}
          </span>
        {/if}
        {#if loading}
          <span class="text-sky-400 flex items-center gap-1">
            <span class="w-2.5 h-2.5 border border-sky-500 border-t-transparent rounded-full animate-spin inline-block"></span>
            {t('grid.loading')}
          </span>
        {/if}
      </div>

      <div class="flex items-center gap-1">
        <select
          class="bg-[#121214] border border-[#2e2e33] rounded px-1.5 py-0.5 text-zinc-300 focus:outline-none focus:border-sky-500"
          title={t('grid.rows_per_page')}
          value={limit}
          onchange={(e) => setLimit(Number((e.target as HTMLSelectElement).value))}
        >
          {#each [50, 100, 250, 500, 1000] as n}
            <option value={n}>{n}</option>
          {/each}
        </select>

        <button
          class="p-1 rounded hover:bg-[#27272a] disabled:opacity-30"
          disabled={currentPage <= 1 || loading}
          onclick={() => goToPage(1)}
          title={t('grid.first_page')}
        >
          <ChevronsLeft class="w-3.5 h-3.5" />
        </button>
        <button
          class="p-1 rounded hover:bg-[#27272a] disabled:opacity-30"
          disabled={currentPage <= 1 || loading}
          onclick={() => goToPage(currentPage - 1)}
          title={t('grid.prev_page')}
        >
          <ChevronLeft class="w-3.5 h-3.5" />
        </button>
        <span class="px-1 text-zinc-400">{t('grid.page_of', { page: currentPage, total: totalPages })}</span>
        <button
          class="p-1 rounded hover:bg-[#27272a] disabled:opacity-30"
          disabled={currentPage >= totalPages || loading}
          onclick={() => goToPage(currentPage + 1)}
          title={t('grid.next_page')}
        >
          <ChevronRight class="w-3.5 h-3.5" />
        </button>
        <button
          class="p-1 rounded hover:bg-[#27272a] disabled:opacity-30"
          disabled={currentPage >= totalPages || loading}
          onclick={() => goToPage(totalPages)}
          title={t('grid.last_page')}
        >
          <ChevronsRight class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  {/if}

  <!-- TablePlus Signature Floating Commit / Discard Banner -->
  {#if totalStaged > 0}
    <div class="absolute bottom-3 left-1/2 -translate-x-1/2 bg-[#1c1c20] border border-amber-500/50 shadow-2xl rounded-lg px-4 py-2 flex items-center gap-4 z-20 animate-in fade-in slide-in-from-bottom-2">
      <div class="flex items-center gap-2">
        <span class="w-2.5 h-2.5 rounded-full bg-amber-400 animate-ping"></span>
        <span class="font-semibold text-zinc-100">
          {t('grid.pending_changes', { count: totalStaged })}
        </span>
        <span class="text-[10px] text-zinc-400">
          {t('grid.pending_breakdown', { inserts: staged.inserts.length, updates: staged.updates.length, deletes: staged.deletes.length })}
        </span>
      </div>

      <div class="flex items-center gap-2">
        <button
          class="px-2.5 py-1 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-300 transition-colors flex items-center gap-1"
          onclick={requestDiscard}
          title={t('grid.discard_hint')}
        >
          <RotateCcw class="w-3.5 h-3.5" />
          <span>{t('common.discard')}</span>
        </button>

        <button
          class="px-3 py-1 rounded bg-[#27272a] hover:bg-[#3f3f46] text-sky-400 border border-sky-500/30 transition-colors flex items-center gap-1 font-medium"
          onclick={() => workspace.previewStaged(tableName, primaryKeys)}
          title={t('grid.review_hint')}
        >
          <Code class="w-3.5 h-3.5" />
          <span>{t('grid.review')}</span>
        </button>

        <button
          class="px-3.5 py-1 rounded bg-emerald-600 hover:bg-emerald-500 text-white font-semibold transition-colors flex items-center gap-1 shadow-md"
          onclick={() => workspace.commitStaged(tableName, primaryKeys)}
          title={t('grid.commit_hint')}
        >
          <Save class="w-3.5 h-3.5" />
          <span>{t('grid.commit')}</span>
        </button>
      </div>
    </div>
  {/if}

  <!-- Row Detail Drawer (Spacebar) -->
  {#if detailDrawerRow && tableData}
    <div class="absolute right-0 top-0 bottom-0 w-80 bg-[#18181b] border-l border-[#27272a] shadow-2xl flex flex-col z-30 animate-in slide-in-from-right duration-150">
      <div class="h-10 border-b border-[#27272a] px-3 flex items-center justify-between text-zinc-200 font-semibold">
        <span>{t('grid.row_detail')}</span>
        <button class="p-1 rounded hover:bg-[#27272a]" onclick={() => (detailDrawerRow = null)}>
          <X class="w-4 h-4 text-zinc-400" />
        </button>
      </div>
      <div class="flex-1 overflow-y-auto p-3 space-y-3">
        {#each tableData.columns as col, i}
          <div class="space-y-1">
            <label class="text-[10px] text-zinc-400 font-medium uppercase tracking-wider block">{col}</label>
            <div class="p-2 rounded bg-[#121214] border border-[#27272a] text-zinc-200 font-mono text-[11px] break-all select-text">
              {detailDrawerRow[i] === null ? 'NULL' : detailDrawerRow[i]}
            </div>
          </div>
        {/each}
      </div>
      <!-- Row actions (Workbench-style: copy row as INSERT / JSON) -->
      <div class="p-3 border-t border-[#27272a] flex gap-2">
        <button
          class="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-200 text-[11px] transition-colors"
          onclick={() => copyRowAs('insert')}
        >
          <Copy class="w-3 h-3 text-emerald-400" />
          <span>{copiedKind === 'insert' ? t('common.copied') : t('grid.copy_insert')}</span>
        </button>
        <button
          class="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-200 text-[11px] transition-colors"
          onclick={() => copyRowAs('json')}
        >
          <Copy class="w-3 h-3 text-sky-400" />
          <span>{copiedKind === 'json' ? t('common.copied') : t('grid.copy_json')}</span>
        </button>
      </div>
    </div>
  {/if}

  <!-- Bulk Edit Modal (HeidiSQL Feature) -->
  {#if isBulkEditOpen && tableData}
    <div class="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center z-50">
      <div class="w-96 bg-[#18181b] border border-[#27272a] rounded-lg shadow-2xl p-4 space-y-4">
        <h3 class="font-bold text-zinc-100 text-sm">{t('grid.bulk_title', { count: selectedRows.size })}</h3>
        <p class="text-zinc-400 text-[11px]">
          {t('grid.bulk_desc')}
        </p>

        <div class="space-y-1">
          <label class="text-[10px] text-zinc-400 uppercase font-semibold">{t('grid.bulk_column')}</label>
          <select
            bind:value={bulkEditColumn}
            class="w-full bg-[#121214] border border-[#27272a] rounded px-2 py-1.5 text-zinc-200 focus:outline-none focus:border-sky-500"
          >
            {#each tableData.columns as col}
              <option value={col}>{col}</option>
            {/each}
          </select>
        </div>

        <div class="space-y-1">
          <label class="text-[10px] text-zinc-400 uppercase font-semibold">{t('grid.bulk_value')}</label>
          <input
            type="text"
            bind:value={bulkEditValue}
            placeholder={t('grid.bulk_value_ph')}
            class="w-full bg-[#121214] border border-[#27272a] rounded px-2.5 py-1.5 text-zinc-200 focus:outline-none focus:border-sky-500 font-mono text-[11px]"
          />
        </div>

        <div class="flex justify-end gap-2 pt-2">
          <button
            class="px-3 py-1 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-300"
            onclick={() => (isBulkEditOpen = false)}
          >
            {t('common.cancel')}
          </button>
          <button
            class="px-4 py-1 rounded bg-amber-600 hover:bg-amber-500 text-white font-semibold"
            onclick={applyBulkEdit}
          >
            {t('grid.bulk_apply')}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  @keyframes datagrid-slide {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(300%);
    }
  }
</style>
