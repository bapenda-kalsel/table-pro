<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import { withProdGuard } from '$lib/api/approval';
  import { Wrench, CheckCircle2, X, Archive, FolderOpen } from 'lucide-svelte';
  import { openPath } from '@tauri-apps/plugin-opener';
  import { t } from '$lib/i18n/i18n.svelte';
  import { toasts } from '$lib/stores/toast.svelte';

  let selectedAction = $state<'vacuum' | 'integrity_check' | 'analyze' | 'reindex' | 'backup'>('vacuum');
  let isRunning = $state(false);
  let outputResult = $state<string | null>(null);
  let backupPath = $state<string | null>(null);

  let actions = $derived<{ key: typeof selectedAction; title: string; desc: string }[]>([
    { key: 'vacuum', title: t('mt.act_vacuum'), desc: t('mt.act_vacuum_desc') },
    { key: 'integrity_check', title: t('mt.act_integrity'), desc: t('mt.act_integrity_desc') },
    { key: 'analyze', title: t('mt.act_analyze'), desc: t('mt.act_analyze_desc') },
    { key: 'reindex', title: t('mt.act_reindex'), desc: t('mt.act_reindex_desc') },
    { key: 'backup', title: t('mt.act_backup'), desc: t('mt.act_backup_desc') },
  ]);

  async function executeMaintenance() {
    if (!workspace.activeConnection) return;
    isRunning = true;
    outputResult = null;
    backupPath = null;
    try {
      if (selectedAction === 'backup') {
        backupPath = await api.backupDatabase(workspace.activeConnection.id);
        outputResult = t('mt.backup_done', { path: backupPath });
      } else {
        outputResult = await withProdGuard((tok) =>
          api.runMaintenance(workspace.activeConnection!.id, selectedAction, undefined, tok)
        );
      }
      await workspace.refreshLogs();
    } catch (err: any) {
      outputResult = 'Error: ' + err;
      await workspace.refreshLogs();
    } finally {
      isRunning = false;
    }
  }

  async function openBackupsFolder() {
    try {
      await openPath(await api.getDataDir('backups'));
    } catch (err: any) {
      toasts.error(t('mt.open_failed') + err);
    }
  }
</script>

{#if workspace.isMaintenanceModalOpen}
  <div class="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center z-50 p-4">
    <div class="w-[540px] bg-panel border border-edge rounded-xl shadow-2xl flex flex-col text-xs overflow-hidden select-none">
      <!-- Header -->
      <div class="p-4 border-b border-edge flex items-center justify-between bg-subtle">
        <div class="flex items-center gap-2.5">
          <div class="p-1.5 rounded-md bg-accent-soft border border-accent-edge text-accent">
            <Wrench class="w-4 h-4" />
          </div>
          <div>
            <h3 class="font-bold text-fg text-sm">{t('mt.title')}</h3>
            <p class="text-muted text-[11px]">{t('mt.subtitle')}</p>
          </div>
        </div>
        <button
          class="p-1 rounded hover:bg-hover text-muted hover:text-fg"
          onclick={() => (workspace.isMaintenanceModalOpen = false)}
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Action Selection -->
      <div class="p-4 space-y-2.5 max-h-[380px] overflow-y-auto">
        <div class="block font-semibold text-fg2">{t('mt.choose')}</div>

        {#each actions as action}
          <label class={`flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition-colors ${selectedAction === action.key ? 'border-accent-edge bg-accent-soft/40' : 'border-edge hover:bg-elevated'}`}>
            <input
              type="radio"
              name="maintenance-action"
              value={action.key}
              checked={selectedAction === action.key}
              onchange={() => (selectedAction = action.key)}
              class="mt-1 accent-accent"
            />
            <div>
              <div class="font-semibold text-fg flex items-center gap-1.5">
                {#if action.key === 'backup'}
                  <Archive class="w-3.5 h-3.5 text-accent" />
                {/if}
                {action.title}
              </div>
              <div class="text-[11px] text-muted">{action.desc}</div>
            </div>
          </label>
        {/each}

        {#if outputResult}
          <div class="p-3 rounded-md bg-app border border-edge font-mono text-[11px] text-ok select-text whitespace-pre-wrap">
            <div class="font-bold text-fg2 mb-1">{t('mt.result')}</div>
            {outputResult}
          </div>
        {/if}
        {#if backupPath}
          <button
            class="flex items-center gap-1.5 text-[11px] text-accent hover:underline"
            onclick={openBackupsFolder}
          >
            <FolderOpen class="w-3.5 h-3.5" /> {t('mt.open_backups')}
          </button>
        {/if}
      </div>

      <!-- Footer -->
      <div class="p-3 border-t border-edge bg-subtle flex justify-end gap-2">
        <button
          class="px-3 py-1.5 rounded-md hover:bg-hover text-muted hover:text-fg"
          onclick={() => (workspace.isMaintenanceModalOpen = false)}
        >
          {t('common.close')}
        </button>
        <button
          class="px-4 py-1.5 rounded-md bg-accent-strong hover:opacity-95 text-white font-semibold transition-colors disabled:opacity-50"
          onclick={executeMaintenance}
          disabled={isRunning}
        >
          {isRunning ? t('mt.running') : t('mt.execute')}
        </button>
      </div>
    </div>
  </div>
{/if}
