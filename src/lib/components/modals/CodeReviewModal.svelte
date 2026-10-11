<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { Code, Check, X, AlertTriangle, Save, RotateCcw } from 'lucide-svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  function handleCommit() {
    if (workspace.reviewTable) {
      workspace.commitStaged(workspace.reviewTable, workspace.reviewPrimaryKeys);
    }
  }

  function handleDiscard() {
    if (workspace.reviewTable) {
      workspace.discardChanges(workspace.reviewTable);
      workspace.isCodeReviewModalOpen = false;
    }
  }
</script>

{#if workspace.isCodeReviewModalOpen && workspace.stagedDiff}
  <div class="fixed inset-0 bg-black/70 backdrop-blur-xs flex items-center justify-center z-50 p-4">
    <div class="w-[680px] max-h-[85vh] bg-panel border border-edge rounded-xl shadow-2xl flex flex-col text-xs overflow-hidden select-none">
      <!-- Modal Header -->
      <div class="p-4 border-b border-edge flex items-center justify-between bg-subtle">
        <div class="flex items-center gap-2.5">
          <div class="p-2 rounded bg-sky-500/20 text-sky-400">
            <Code class="w-4 h-4" />
          </div>
          <div>
            <h3 class="font-bold text-fg text-sm">{t('review.title')}</h3>
            <p class="text-muted text-[11px]">
              {t('review.subtitle')}
            </p>
          </div>
        </div>
        <button
          class="p-1 rounded hover:bg-hover text-muted hover:text-fg"
          onclick={() => (workspace.isCodeReviewModalOpen = false)}
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Production warning -->
      {#if workspace.activeConnection?.environment === 'production'}
        <div class="px-4 py-2 bg-rose-950/50 border-b border-rose-900/50 flex items-center gap-2 text-[11px] text-rose-300">
          <AlertTriangle class="w-3.5 h-3.5 shrink-0" />
          <span class="font-semibold">{t('review.prod_warn', { name: workspace.activeConnection.name })}</span>
        </div>
      {/if}

      <!-- Stats Summary Banner -->
      <div class="px-4 py-2.5 border-b border-edge bg-subtle flex items-center gap-4 text-[11px] font-mono">
        <span class="text-emerald-400 font-semibold">{t('review.inserts', { count: workspace.stagedDiff.insert_count })}</span>
        <span class="text-amber-400 font-semibold">{t('review.updates', { count: workspace.stagedDiff.update_count })}</span>
        <span class="text-rose-400 font-semibold">{t('review.deletes', { count: workspace.stagedDiff.delete_count })}</span>
        <span class="text-faint ml-auto font-sans text-[10px]">{t('review.atomic')}</span>
      </div>

      <!-- SQL Statements List -->
      <div class="flex-1 overflow-y-auto p-4 space-y-2 font-mono text-[11px] bg-app">
        {#if workspace.stagedDiff.generated_sql.length === 0}
          <div class="text-faint italic p-4 text-center">{t('review.empty')}</div>
        {:else}
          {#each workspace.stagedDiff.generated_sql as sql, i}
            {@const isInsert = sql.startsWith('INSERT')}
            {@const isUpdate = sql.startsWith('UPDATE')}
            {@const isDelete = sql.startsWith('DELETE')}

            <div
              class={`p-2.5 rounded border select-text break-all ${
                isInsert
                  ? 'bg-emerald-950/20 border-emerald-900/40 text-emerald-300'
                  : isUpdate
                  ? 'bg-amber-950/20 border-amber-900/40 text-amber-300'
                  : isDelete
                  ? 'bg-rose-950/20 border-rose-900/40 text-rose-300'
                  : 'bg-panel border-edge text-fg'
              }`}
            >
              <div class="flex items-center gap-2 mb-1 opacity-60 text-[10px]">
                <span>#{i + 1}</span>
                <span>{isInsert ? 'INSERT' : isUpdate ? 'UPDATE' : 'DELETE'}</span>
              </div>
              <div>{sql}</div>
            </div>
          {/each}
        {/if}
      </div>

      <!-- Modal Footer -->
      <div class="p-3 border-t border-edge bg-panel flex items-center justify-between">
        <button
          class="flex items-center gap-1.5 px-3 py-1.5 rounded bg-elevated hover:bg-hover text-fg2 font-medium transition-colors"
          onclick={handleDiscard}
        >
          <RotateCcw class="w-3.5 h-3.5" />
          <span>{t('review.discard')}</span>
        </button>

        <div class="flex items-center gap-2">
          <button
            class="px-3 py-1.5 rounded hover:bg-hover text-muted hover:text-fg"
            onclick={() => (workspace.isCodeReviewModalOpen = false)}
          >
            {t('common.cancel')}
          </button>
          <button
            class="flex items-center gap-1.5 px-4 py-1.5 rounded bg-emerald-600 hover:bg-emerald-500 text-white font-semibold shadow-md transition-colors"
            onclick={handleCommit}
          >
            <Save class="w-3.5 h-3.5" />
            <span>{t('review.commit')}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
