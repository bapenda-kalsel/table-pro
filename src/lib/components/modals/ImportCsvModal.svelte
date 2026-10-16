<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import { withProdGuard } from '$lib/api/approval';
  import { FileUp, X, FileText, CheckCircle2 } from 'lucide-svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { t } from '$lib/i18n/i18n.svelte';

  let filePath = $state('');
  let targetTable = $state('');
  let isImporting = $state(false);
  let resultMsg = $state<string | null>(null);
  let errorMsg = $state<string | null>(null);

  const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

  async function pickFile() {
    if (!isTauri) {
      filePath = '/mock/path/data.csv';
      if (!targetTable) targetTable = 'data';
      return;
    }
    const selected = await open({
      title: t('ic.pick_title'),
      multiple: false,
      filters: [{ name: 'CSV', extensions: ['csv', 'tsv', 'txt'] }],
    });
    if (typeof selected === 'string') {
      filePath = selected;
      // Suggest a table name from the file stem.
      if (!targetTable) {
        const stem = selected.split('/').pop()?.replace(/\.[^.]+$/, '') ?? '';
        targetTable = stem.replace(/[^a-zA-Z0-9_]/g, '_');
      }
    }
  }

  async function runImport() {
    if (!workspace.activeConnection || !filePath || !targetTable) return;
    isImporting = true;
    resultMsg = null;
    errorMsg = null;
    try {
      const count = await withProdGuard((tok) =>
        api.importCsv(workspace.activeConnection!.id, targetTable, filePath, tok)
      );
      resultMsg = t('ic.imported', { count, table: targetTable });
      await workspace.refreshTables();
      await workspace.refreshSchemaCache();
      await workspace.refreshLogs();
    } catch (err: any) {
      errorMsg = String(err);
    } finally {
      isImporting = false;
    }
  }

  function close() {
    workspace.isImportCsvModalOpen = false;
    filePath = '';
    targetTable = '';
    resultMsg = null;
    errorMsg = null;
  }
</script>

{#if workspace.isImportCsvModalOpen}
  <div class="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center z-50 p-4">
    <div class="w-[480px] bg-panel border border-edge rounded-xl shadow-2xl flex flex-col text-xs overflow-hidden select-none">
      <!-- Header -->
      <div class="p-4 border-b border-edge flex items-center justify-between bg-subtle">
        <div class="flex items-center gap-2.5">
          <div class="p-1.5 rounded-md bg-accent-soft border border-accent-edge text-accent">
            <FileUp class="w-4 h-4" />
          </div>
          <div>
            <h3 class="font-bold text-fg text-sm">{t('ic.title')}</h3>
            <p class="text-muted text-[11px]">{t('ic.subtitle')}</p>
          </div>
        </div>
        <button class="p-1 rounded hover:bg-hover text-muted hover:text-fg" onclick={close}>
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Body -->
      <div class="p-4 space-y-4">
        <div class="space-y-1.5">
          <label for="ic-file" class="text-[10px] text-muted uppercase font-semibold block">{t('ic.file')}</label>
          <div class="flex gap-2">
            <input
              id="ic-file"
              type="text"
              bind:value={filePath}
              placeholder="/path/to/data.csv"
              class="flex-1 bg-input border border-edge rounded-md px-3 py-1.5 text-fg focus:outline-none focus:border-accent font-mono text-[11px]"
            />
            <button
              class="flex items-center gap-1 px-3 py-1.5 rounded-md bg-elevated hover:bg-hover border border-edge text-fg transition-colors"
              onclick={pickFile}
            >
              <FileText class="w-3.5 h-3.5" /><span>{t('common.browse')}</span>
            </button>
          </div>
        </div>

        <div class="space-y-1.5">
          <label for="ic-table" class="text-[10px] text-muted uppercase font-semibold block">{t('ic.table')}</label>
          <input
            id="ic-table"
            type="text"
            bind:value={targetTable}
            placeholder={t('ic.table_ph')}
            class="w-full bg-input border border-edge rounded-md px-3 py-1.5 text-fg focus:outline-none focus:border-accent font-mono text-[11px]"
          />
          <p class="text-[10px] text-faint">
            {t('ic.table_note')}
          </p>
        </div>

        {#if resultMsg}
          <div class="p-2.5 rounded-md border bg-ok-bg border-ok-edge text-ok text-[11px] flex items-center gap-2">
            <CheckCircle2 class="w-4 h-4 shrink-0" /><span>{resultMsg}</span>
          </div>
        {/if}
        {#if errorMsg}
          <div class="p-2.5 rounded-md border bg-err-bg border-err-edge text-err text-[11px] font-mono select-text">
            {errorMsg}
          </div>
        {/if}
      </div>

      <!-- Footer -->
      <div class="p-3 border-t border-edge bg-subtle flex justify-end gap-2">
        <button class="px-3 py-1.5 rounded-md hover:bg-hover text-muted hover:text-fg" onclick={close}>
          {t('common.close')}
        </button>
        <button
          class="px-4 py-1.5 rounded-md bg-accent-strong hover:opacity-95 text-white font-semibold transition-colors disabled:opacity-50"
          onclick={runImport}
          disabled={isImporting || !filePath || !targetTable}
        >
          {isImporting ? t('ic.importing') : t('ic.import')}
        </button>
      </div>
    </div>
  </div>
{/if}
