<script lang="ts">
  import { toasts } from '$lib/stores/toast.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { AlertCircle, CheckCircle2, Info, X } from 'lucide-svelte';
</script>

{#if toasts.items.length > 0}
  <div class="fixed bottom-4 right-4 z-[70] flex flex-col gap-2 w-80 select-none">
    {#each toasts.items as toast (toast.id)}
      <div
        class={`flex items-start gap-2.5 rounded-lg border px-3 py-2.5 shadow-2xl text-[11px] bg-panel ${
          toast.kind === 'error'
            ? 'border-err-edge text-fg'
            : toast.kind === 'success'
              ? 'border-ok-edge text-fg'
              : 'border-strong text-fg'
        }`}
        role="alert"
      >
        {#if toast.kind === 'error'}
          <AlertCircle class="w-3.5 h-3.5 mt-0.5 shrink-0 text-err" />
        {:else if toast.kind === 'success'}
          <CheckCircle2 class="w-3.5 h-3.5 mt-0.5 shrink-0 text-ok" />
        {:else}
          <Info class="w-3.5 h-3.5 mt-0.5 shrink-0 text-accent" />
        {/if}
        <span class="flex-1 break-words select-text leading-relaxed">{toast.message}</span>
        <button
          class="p-0.5 rounded hover:bg-hover text-muted hover:text-fg shrink-0"
          onclick={() => toasts.dismiss(toast.id)}
          title={t('common.dismiss')}
        >
          <X class="w-3 h-3" />
        </button>
      </div>
    {/each}
  </div>
{/if}
