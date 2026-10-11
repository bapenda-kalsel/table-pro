<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import type { FullTableSchema } from '$lib/types';
  import { FileCode, Copy, Check, KeyRound, Link, Zap, Table2 } from 'lucide-svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  type SectionKey = 'columns' | 'indexes' | 'fks' | 'triggers' | 'ddl';

  let { tableName }: { tableName: string } = $props();

  let schema = $state<FullTableSchema | null>(null);
  let copied = $state(false);
  let loadError = $state<string | null>(null);
  let activeSection = $state<SectionKey>('columns');

  let sections = $derived<{ key: SectionKey; label: string; icon: any }[]>([
    { key: 'columns', label: `${t('structure.sec_columns')} (${schema?.columns.length ?? 0})`, icon: Table2 },
    { key: 'indexes', label: `${t('structure.sec_indexes')} (${schema?.indexes.length ?? 0})`, icon: KeyRound },
    { key: 'fks', label: `${t('structure.sec_fks')} (${schema?.foreign_keys.length ?? 0})`, icon: Link },
    { key: 'triggers', label: `${t('structure.sec_triggers')} (${schema?.triggers.length ?? 0})`, icon: Zap },
    { key: 'ddl', label: 'DDL', icon: FileCode },
  ]);

  $effect(() => {
    if (tableName && workspace.activeConnection) {
      loadStructure();
    }
  });

  async function loadStructure() {
    if (!workspace.activeConnection) return;
    loadError = null;
    try {
      schema = await api.getFullSchema(workspace.activeConnection.id, tableName);
    } catch (err: any) {
      loadError = String(err);
    }
  }

  function copyDdl() {
    if (!schema?.ddl) return;
    navigator.clipboard.writeText(schema.ddl);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }

</script>

<div class="flex-1 flex flex-col h-full bg-app text-xs select-none overflow-hidden">
  <!-- Header -->
  <div class="h-10 border-b border-edge bg-panel flex items-center justify-between px-4">
    <div class="flex items-center gap-2 font-semibold text-fg">
      <FileCode class="w-4 h-4 text-amber-400" />
      <span>{t('structure.title', { table: tableName })}</span>
    </div>
    <button
      class="flex items-center gap-1.5 px-2.5 py-1 rounded bg-elevated hover:bg-hover text-fg transition-colors"
      onclick={copyDdl}
    >
      {#if copied}
        <Check class="w-3.5 h-3.5 text-emerald-400" />
        <span class="text-emerald-400">{t('structure.copied')}</span>
      {:else}
        <Copy class="w-3.5 h-3.5" />
        <span>{t('structure.copy')}</span>
      {/if}
    </button>
  </div>

  <!-- Section tabs -->
  <div class="h-9 border-b border-edge bg-subtle flex items-center px-2 gap-1">
    {#each sections as s}
      {@const Icon = s.icon}
      <button
        class={`flex items-center gap-1.5 px-3 py-1.5 rounded text-[11px] font-medium transition-colors ${
          activeSection === s.key
            ? 'bg-elevated text-fg border border-strong'
            : 'text-muted hover:text-fg hover:bg-elevated'
        }`}
        onclick={() => (activeSection = s.key)}
      >
        <Icon class="w-3.5 h-3.5" />
        <span>{s.label}</span>
      </button>
    {/each}
  </div>

  <div class="flex-1 overflow-auto p-4">
    {#if loadError}
      <div class="p-4 bg-rose-950/40 border-l-4 border-rose-500 text-rose-300 font-mono text-xs">{loadError}</div>
    {:else if !schema}
      <div class="flex items-center justify-center h-32 text-faint gap-2">
        <div class="w-4 h-4 border-2 border-sky-500 border-t-transparent rounded-full animate-spin"></div>
        <span>{t('structure.loading')}</span>
      </div>
    {:else}

      {#if activeSection === 'columns'}
        <div class="border border-edge rounded-lg overflow-hidden bg-panel">
          <table class="w-full text-left font-mono text-[11px]">
            <thead class="bg-elevated text-muted border-b border-edge">
              <tr>
                <th class="px-3 py-2">{t('structure.col_name')}</th>
                <th class="px-3 py-2">{t('structure.col_type')}</th>
                <th class="px-3 py-2">{t('structure.col_nullable')}</th>
                <th class="px-3 py-2">{t('structure.col_pk')}</th>
                <th class="px-3 py-2">{t('structure.col_default')}</th>
              </tr>
            </thead>
            <tbody>
              {#each schema.columns as col}
                <tr class="border-b border-edge/50 hover:bg-hover">
                  <td class="px-3 py-2 font-semibold text-fg">
                    <span class="flex items-center gap-1.5">
                      {#if col.is_primary_key}
                        <span class="text-amber-400 font-bold" title={t('structure.col_pk')}>🔑</span>
                      {/if}
                      <span>{col.name}</span>
                    </span>
                  </td>
                  <td class="px-3 py-2 text-sky-400">{col.data_type || t('structure.no_type')}</td>
                  <td class="px-3 py-2 text-muted">{col.is_nullable ? 'YES' : 'NO'}</td>
                  <td class="px-3 py-2">
                    <span class={col.is_primary_key ? 'text-amber-400 font-bold' : 'text-faint'}>
                      {col.is_primary_key ? 'PRIMARY KEY' : '-'}
                    </span>
                  </td>
                  <td class="px-3 py-2 text-muted">{col.default_value || '-'}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>

      {:else if activeSection === 'indexes'}
        {#if schema.indexes.length === 0}
          <div class="p-8 text-center text-faint italic">{t('structure.no_indexes')}</div>
        {:else}
          <div class="border border-edge rounded-lg overflow-hidden bg-panel">
            <table class="w-full text-left font-mono text-[11px]">
              <thead class="bg-elevated text-muted border-b border-edge">
                <tr>
                  <th class="px-3 py-2">{t('structure.idx_name')}</th>
                  <th class="px-3 py-2">{t('structure.idx_columns')}</th>
                  <th class="px-3 py-2">{t('structure.idx_unique')}</th>
                  <th class="px-3 py-2">{t('structure.idx_origin')}</th>
                  <th class="px-3 py-2">{t('structure.idx_partial')}</th>
                </tr>
              </thead>
              <tbody>
                {#each schema.indexes as idx}
                  <tr class="border-b border-edge/50 hover:bg-hover">
                    <td class="px-3 py-2 font-semibold text-fg">{idx.name}</td>
                    <td class="px-3 py-2 text-sky-400">{idx.columns.join(', ') || '-'}</td>
                    <td class="px-3 py-2">
                      <span class={idx.is_unique ? 'text-emerald-400 font-bold' : 'text-faint'}>
                        {idx.is_unique ? 'UNIQUE' : '-'}
                      </span>
                    </td>
                    <td class="px-3 py-2 text-muted">
                      {idx.origin === 'pk' ? t('structure.origin_pk') : idx.origin === 'u' ? t('structure.origin_uq') : t('structure.origin_idx')}
                    </td>
                    <td class="px-3 py-2 text-muted">{idx.is_partial ? 'PARTIAL' : '-'}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}

      {:else if activeSection === 'fks'}
        {#if schema.foreign_keys.length === 0}
          <div class="p-8 text-center text-faint italic">{t('structure.no_fks')}</div>
        {:else}
          <div class="border border-edge rounded-lg overflow-hidden bg-panel">
            <table class="w-full text-left font-mono text-[11px]">
              <thead class="bg-elevated text-muted border-b border-edge">
                <tr>
                  <th class="px-3 py-2">{t('structure.fk_column')}</th>
                  <th class="px-3 py-2">{t('structure.fk_references')}</th>
                  <th class="px-3 py-2">ON UPDATE</th>
                  <th class="px-3 py-2">ON DELETE</th>
                </tr>
              </thead>
              <tbody>
                {#each schema.foreign_keys as fk}
                  <tr class="border-b border-edge/50 hover:bg-hover">
                    <td class="px-3 py-2 text-fg font-semibold">{fk.from_column}</td>
                    <td class="px-3 py-2 text-sky-400">
                      <button class="hover:underline" onclick={() => workspace.openTableTab(fk.referenced_table)}>
                        {fk.referenced_table}
                      </button>.{fk.to_column ?? '?'}
                    </td>
                    <td class="px-3 py-2 text-muted">{fk.on_update}</td>
                    <td class="px-3 py-2 text-muted">{fk.on_delete}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}

      {:else if activeSection === 'triggers'}
        {#if schema.triggers.length === 0}
          <div class="p-8 text-center text-faint italic">{t('structure.no_triggers')}</div>
        {:else}
          <div class="space-y-3">
            {#each schema.triggers as trig}
              <div class="border border-edge rounded-lg overflow-hidden bg-panel">
                <div class="p-2.5 border-b border-edge flex items-center gap-2">
                  <Zap class="w-3.5 h-3.5 text-violet-400" />
                  <span class="font-semibold text-fg">{trig.name}</span>
                </div>
                <pre class="p-3 font-mono text-[11px] text-fg2 overflow-x-auto select-text">{trig.sql}</pre>
              </div>
            {/each}
          </div>
        {/if}

      {:else if activeSection === 'ddl'}
        <div class="border border-edge rounded-lg overflow-hidden bg-panel">
          <div class="p-3 border-b border-edge font-semibold text-fg2">
            {t('structure.ddl_title')}
          </div>
          <pre class="p-4 font-mono text-[11px] text-fg2 bg-app overflow-x-auto select-text">{schema.ddl ?? t('structure.no_ddl')}</pre>
        </div>
      {/if}
    {/if}
  </div>
</div>
