<script lang="ts">
  import { toasts } from '$lib/stores/toast.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { AlertCircle, CheckCircle2, Info, X } from 'lucide-svelte';
</script>

{#if toasts.items.length > 0}
  <div class="fixed bottom-4 right-4 z-[70] flex flex-col gap-2 w-80 select-none">
    {#each toasts.items as toast (toast.id)}
      <div
        class={`flex items-start gap-2 rounded-lg border px-3 py-2 shadow-2xl text-[11px] animate-in slide-in-from-right duration-150 ${
          toast.kind === 'error'
            ? 'bg-rose-950/90 border-rose-700/50 text-rose-200'
            : toast.kind === 'success'
              ? 'bg-emerald-950/90 border-emerald-700/50 text-emerald-200'
              : 'bg-elevated/95 border-strong text-fg'
        }`}
        role="alert"
      >
        {#if toast.kind === 'error'}
          <AlertCircle class="w-3.5 h-3.5 mt-0.5 shrink-0 text-rose-400" />
        {:else if toast.kind === 'success'}
          <CheckCircle2 class="w-3.5 h-3.5 mt-0.5 shrink-0 text-emerald-400" />
        {:else}
          <Info class="w-3.5 h-3.5 mt-0.5 shrink-0 text-sky-400" />
        {/if}
        <span class="flex-1 break-words select-text">{toast.message}</span>
        <button
          class="p-0.5 rounded hover:bg-white/10 shrink-0"
          onclick={() => toasts.dismiss(toast.id)}
          title={t('common.dismiss')}
        >
          <X class="w-3 h-3" />
        </button>
      </div>
    {/each}
  </div>
{/if}
