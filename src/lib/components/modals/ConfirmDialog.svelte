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
    <div class="w-96 bg-[#18181b] border border-[#27272a] rounded-lg shadow-2xl p-4 space-y-3 text-xs select-none" role="alertdialog">
      <div class="flex items-center gap-2">
        <AlertTriangle class={`w-4 h-4 shrink-0 ${req.danger ? 'text-amber-400' : 'text-sky-400'}`} />
        <h3 class="font-bold text-zinc-100 text-sm">{req.title}</h3>
      </div>
      {#if req.message}
        <div class="text-zinc-300 text-[11px] whitespace-pre-wrap break-all max-h-48 overflow-y-auto select-text font-mono">
          {req.message}
        </div>
      {/if}
      <div class="flex justify-end gap-2 pt-1">
        <button
          class="px-3 py-1 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-300 transition-colors"
          onclick={() => confirmDialog.answer(false)}
        >
          {t('common.cancel')}
        </button>
        <button
          class={`px-4 py-1 rounded text-white font-semibold transition-colors ${
            req.danger ? 'bg-rose-600 hover:bg-rose-500' : 'bg-sky-600 hover:bg-sky-500'
          }`}
          onclick={() => confirmDialog.answer(true)}
        >
          {req.confirmLabel ?? t('confirm.confirm')}
        </button>
      </div>
    </div>
  </div>
{/if}
