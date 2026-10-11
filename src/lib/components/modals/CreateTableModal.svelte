<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import { buildCreateTableSql, SQLITE_TYPES, type ColumnDef } from '$lib/utils/sql';
  import { Table2, Plus, Trash2, X, AlertCircle, Play } from 'lucide-svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  let tableName = $state('new_table');
  let columns = $state<ColumnDef[]>([
    { name: 'id', dataType: 'INTEGER', isPrimaryKey: true, isNotNull: false, isUnique: false, autoIncrement: true, defaultValue: '' },
    { name: 'name', dataType: 'TEXT', isPrimaryKey: false, isNotNull: true, isUnique: false, autoIncrement: false, defaultValue: '' },
  ]);
  let execError = $state<string | null>(null);
  let isCreating = $state(false);

  let preview = $derived.by(() => {
    try {
      return { sql: buildCreateTableSql(tableName, columns), error: null };
    } catch (err: any) {
      return { sql: '', error: String(err?.message ?? err) };
    }
  });

  function addColumn() {
    columns.push({
      name: '',
      dataType: 'TEXT',
      isPrimaryKey: false,
      isNotNull: false,
      isUnique: false,
      autoIncrement: false,
      defaultValue: '',
    });
  }

  function removeColumn(i: number) {
    columns.splice(i, 1);
  }

  async function createTable() {
    if (!workspace.activeConnection || preview.error || !preview.sql) return;
    isCreating = true;
    execError = null;
    try {
      await api.executeQuery(workspace.activeConnection.id, preview.sql);
      await workspace.refreshTables();
      await workspace.refreshSchemaCache();
      await workspace.refreshLogs();
      workspace.isCreateTableModalOpen = false;
      workspace.openStructureTab(tableName);
    } catch (err: any) {
      execError = String(err);
    } finally {
      isCreating = false;
    }
  }
</script>

{#if workspace.isCreateTableModalOpen}
  <div class="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center z-50 p-4">
    <div class="w-[820px] max-h-[88vh] bg-[#18181b] border border-[#27272a] rounded-xl shadow-2xl flex flex-col text-xs overflow-hidden select-none">
      <!-- Header -->
      <div class="p-4 border-b border-[#27272a] flex items-center justify-between bg-[#141416]">
        <div class="flex items-center gap-2">
          <div class="p-1.5 rounded bg-sky-500/20 text-sky-400">
            <Table2 class="w-4 h-4" />
          </div>
          <div>
            <h3 class="font-bold text-zinc-100 text-sm">{t('ct.title')}</h3>
            <p class="text-zinc-400 text-[11px]">{t('ct.subtitle')}</p>
          </div>
        </div>
        <button class="p-1 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200" onclick={() => (workspace.isCreateTableModalOpen = false)}>
          <X class="w-4 h-4" />
        </button>
      </div>

      <div class="flex-1 overflow-auto p-4 space-y-3">
        <!-- Table name -->
        <div class="space-y-1">
          <span class="text-[10px] text-zinc-400 uppercase font-semibold">{t('ct.name')}</span>
          <input
            type="text"
            bind:value={tableName}
            class="w-72 bg-[#121214] border border-[#2e2e33] rounded px-3 py-1.5 text-zinc-100 focus:outline-none focus:border-sky-500 font-mono text-[11px]"
          />
        </div>

        <!-- Columns editor grid -->
        <div class="border border-[#27272a] rounded-lg overflow-hidden">
          <table class="w-full text-left font-mono text-[11px]">
            <thead class="bg-[#1e1e24] text-zinc-400 border-b border-[#27272a]">
              <tr>
                <th class="px-2 py-2 w-44">{t('ct.col_name')}</th>
                <th class="px-2 py-2 w-28">{t('ct.col_type')}</th>
                <th class="px-2 py-2 w-10 text-center" title={t('ct.pk_hint')}>PK</th>
                <th class="px-2 py-2 w-10 text-center" title={t('ct.nn_hint')}>NN</th>
                <th class="px-2 py-2 w-10 text-center" title={t('ct.uq_hint')}>UQ</th>
                <th class="px-2 py-2 w-10 text-center" title={t('ct.ai_hint')}>AI</th>
                <th class="px-2 py-2">{t('structure.col_default')}</th>
                <th class="px-2 py-2 w-8"></th>
              </tr>
            </thead>
            <tbody>
              {#each columns as c, i}
                <tr class="border-b border-[#27272a]/50 hover:bg-[#1c1c20]">
                  <td class="px-2 py-1">
                    <input
                      type="text"
                      bind:value={c.name}
                      placeholder="column_name"
                      class="w-full bg-transparent text-zinc-100 focus:outline-none border-b border-transparent focus:border-sky-500 py-0.5"
                    />
                  </td>
                  <td class="px-2 py-1">
                    <select bind:value={c.dataType} class="w-full bg-[#121214] border border-[#2e2e33] rounded px-1 py-0.5 text-zinc-200 focus:outline-none text-[10px]">
                      {#each SQLITE_TYPES as t}
                        <option value={t}>{t}</option>
                      {/each}
                    </select>
                  </td>
                  <td class="px-2 py-1 text-center">
                    <input type="checkbox" bind:checked={c.isPrimaryKey} class="rounded bg-[#27272a] border-zinc-700 accent-amber-400" />
                  </td>
                  <td class="px-2 py-1 text-center">
                    <input type="checkbox" bind:checked={c.isNotNull} class="rounded bg-[#27272a] border-zinc-700 accent-sky-500" />
                  </td>
                  <td class="px-2 py-1 text-center">
                    <input type="checkbox" bind:checked={c.isUnique} class="rounded bg-[#27272a] border-zinc-700 accent-emerald-400" />
                  </td>
                  <td class="px-2 py-1 text-center">
                    <input
                      type="checkbox"
                      bind:checked={c.autoIncrement}
                      disabled={!c.isPrimaryKey || c.dataType !== 'INTEGER'}
                      class="rounded bg-[#27272a] border-zinc-700 accent-violet-400 disabled:opacity-30"
                    />
                  </td>
                  <td class="px-2 py-1">
                    <input
                      type="text"
                      bind:value={c.defaultValue}
                      placeholder={t('ct.default_ph')}
                      class="w-full bg-transparent text-zinc-300 focus:outline-none border-b border-transparent focus:border-sky-500 py-0.5 text-[10px]"
                    />
                  </td>
                  <td class="px-2 py-1 text-center">
                    <button
                      class="p-0.5 rounded hover:bg-rose-500/20 text-zinc-500 hover:text-rose-400"
                      onclick={() => removeColumn(i)}
                      title={t('ct.remove')}
                    >
                      <Trash2 class="w-3 h-3" />
                    </button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
          <button
            class="w-full py-1.5 flex items-center justify-center gap-1 text-zinc-400 hover:text-sky-400 hover:bg-[#1c1c20] transition-colors border-t border-[#27272a]"
            onclick={addColumn}
          >
            <Plus class="w-3.5 h-3.5" /><span>{t('ct.add')}</span>
          </button>
        </div>

        <!-- Live DDL preview -->
        <div class="border border-[#27272a] rounded-lg overflow-hidden bg-[#121214]">
          <div class="px-3 py-2 border-b border-[#27272a] text-[10px] uppercase tracking-wider text-zinc-500 font-semibold">
            {t('ct.ddl')}
          </div>
          {#if preview.error}
            <div class="p-3 text-rose-400 font-mono text-[11px] flex items-center gap-2">
              <AlertCircle class="w-3.5 h-3.5 shrink-0" />{preview.error}
            </div>
          {:else}
            <pre class="p-3 font-mono text-[11px] text-emerald-300 select-text whitespace-pre-wrap">{preview.sql}</pre>
          {/if}
        </div>

        {#if execError}
          <div class="p-3 rounded border bg-rose-950/30 border-rose-900/40 text-rose-300 text-[11px] font-mono select-text">
            {execError}
          </div>
        {/if}
      </div>

      <!-- Footer -->
      <div class="p-3 border-t border-[#27272a] bg-[#141416] flex justify-end gap-2">
        <button class="px-3 py-1.5 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200" onclick={() => (workspace.isCreateTableModalOpen = false)}>
          {t('common.cancel')}
        </button>
        <button
          class="flex items-center gap-1.5 px-4 py-1.5 rounded bg-emerald-600 hover:bg-emerald-500 text-white font-semibold shadow-md transition-colors disabled:opacity-50"
          onclick={createTable}
          disabled={isCreating || !!preview.error}
        >
          <Play class="w-3.5 h-3.5" />
          <span>{isCreating ? t('ct.creating') : t('ct.create')}</span>
        </button>
      </div>
    </div>
  </div>
{/if}
