<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
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
        outputResult = await api.runMaintenance(workspace.activeConnection.id, selectedAction);
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
    <div class="w-[540px] bg-[#18181b] border border-[#27272a] rounded-xl shadow-2xl flex flex-col text-xs overflow-hidden select-none">
      <!-- Header -->
      <div class="p-4 border-b border-[#27272a] flex items-center justify-between bg-[#141416]">
        <div class="flex items-center gap-2">
          <div class="p-1.5 rounded bg-emerald-500/20 text-emerald-400">
            <Wrench class="w-4 h-4" />
          </div>
          <div>
            <h3 class="font-bold text-zinc-100 text-sm">{t('mt.title')}</h3>
            <p class="text-zinc-400 text-[11px]">{t('mt.subtitle')}</p>
          </div>
        </div>
        <button
          class="p-1 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200"
          onclick={() => (workspace.isMaintenanceModalOpen = false)}
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Action Selection -->
      <div class="p-4 space-y-2.5 max-h-[380px] overflow-y-auto">
        <label class="block font-semibold text-zinc-300">{t('mt.choose')}</label>

        {#each actions as action}
          <label class="flex items-start gap-3 p-3 rounded-lg border border-[#27272a] hover:bg-[#1f1f23] cursor-pointer transition-colors">
            <input
              type="radio"
              name="maintenance-action"
              value={action.key}
              checked={selectedAction === action.key}
              onchange={() => (selectedAction = action.key)}
              class="mt-1"
            />
            <div>
              <div class="font-semibold text-zinc-100 flex items-center gap-1.5">
                {#if action.key === 'backup'}
                  <Archive class="w-3.5 h-3.5 text-sky-400" />
                {/if}
                {action.title}
              </div>
              <div class="text-[11px] text-zinc-400">{action.desc}</div>
            </div>
          </label>
        {/each}

        {#if outputResult}
          <div class="p-3 rounded bg-[#121214] border border-[#27272a] font-mono text-[11px] text-emerald-400 select-text whitespace-pre-wrap">
            <div class="font-bold text-zinc-300 mb-1">{t('mt.result')}</div>
            {outputResult}
          </div>
        {/if}
        {#if backupPath}
          <button
            class="flex items-center gap-1.5 text-[11px] text-sky-400 hover:underline"
            onclick={openBackupsFolder}
          >
            <FolderOpen class="w-3.5 h-3.5" /> {t('mt.open_backups')}
          </button>
        {/if}
      </div>

      <!-- Footer -->
      <div class="p-3 border-t border-[#27272a] bg-[#141416] flex justify-end gap-2">
        <button
          class="px-3 py-1.5 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200"
          onclick={() => (workspace.isMaintenanceModalOpen = false)}
        >
          {t('common.close')}
        </button>
        <button
          class="px-4 py-1.5 rounded bg-emerald-600 hover:bg-emerald-500 text-white font-semibold shadow-md transition-colors disabled:opacity-50"
          onclick={executeMaintenance}
          disabled={isRunning}
        >
          {isRunning ? t('mt.running') : t('mt.execute')}
        </button>
      </div>
    </div>
  </div>
{/if}
