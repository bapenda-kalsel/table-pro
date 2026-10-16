<script lang="ts">
  import { confirmDialog } from '$lib/stores/confirm.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { AlertTriangle } from 'lucide-svelte';
</script>

<svelte:window
  onkeydown={(e) => {
    if (confirmDialog.current && e.key === 'Escape') {
      e.preventDefault();
      confirmDialog.answer(false);
    }
  }}
/>

{#if confirmDialog.current}
  {@const req = confirmDialog.current}
  <div class="fixed inset-0 bg-black/60 backdrop-blur-xs flex items-center justify-center z-[60]">
    <div class="w-96 bg-panel border border-edge rounded-xl shadow-2xl p-4 space-y-3 text-xs select-none" role="alertdialog">
      <div class="flex items-center gap-2">
        <AlertTriangle class={`w-4 h-4 shrink-0 ${req.danger ? 'text-err' : 'text-accent'}`} />
        <h3 class="font-bold text-fg text-sm">{req.title}</h3>
      </div>
      {#if req.message}
        <div class="text-fg2 text-[11px] whitespace-pre-wrap break-all max-h-48 overflow-y-auto select-text font-mono">
          {req.message}
        </div>
      {/if}
      <div class="flex justify-end gap-2 pt-1">
        <button
          class="px-3 py-1.5 rounded-md bg-elevated hover:bg-hover border border-edge text-fg2 transition-colors"
          onclick={() => confirmDialog.answer(false)}
        >
          {t('common.cancel')}
        </button>
        <button
          class={`px-4 py-1.5 rounded-md text-white font-semibold transition-colors ${
            req.danger ? 'bg-rose-600 hover:bg-rose-500' : 'bg-accent-strong hover:opacity-95'
          }`}
          onclick={() => confirmDialog.answer(true)}
        >
          {req.confirmLabel ?? t('confirm.confirm')}
        </button>
      </div>
    </div>
  </div>
{/if}
