<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import type { ExplainResult, ExplainStep, QueryResultData } from '$lib/types';
  import { Play, Download, Clock, CheckCircle2, Network, AlertTriangle, ShieldAlert, BookMarked, Trash2, ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from 'lucide-svelte';
  import { firstVerb, isWriteStatement, rowsToCsv } from '$lib/utils/sql';
  import { t } from '$lib/i18n/i18n.svelte';
  import { confirmDialog } from '$lib/stores/confirm.svelte';
  import { onMount } from 'svelte';
  import { EditorView, basicSetup } from 'codemirror';
  import { sql } from '@codemirror/lang-sql';
  import { oneDark } from '@codemirror/theme-one-dark';
  import { keymap } from '@codemirror/view';

  let { initialQuery = 'SELECT * FROM users LIMIT 50;' }: { initialQuery?: string } = $props();

  let editorContainer: HTMLDivElement;
  let editorView: EditorView | null = null;
  let isExecuting = $state(false);
  let isExplaining = $state(false);
  let result = $state<QueryResultData | null>(null);
  let plan = $state<ExplainResult | null>(null);
  let errorMsg = $state<string | null>(null);

  // Result pagination (client-side: executeQuery returns the full result set at once)
  let pageSize = $state(100);
  let page = $state(1);
  let totalPages = $derived(result ? Math.max(1, Math.ceil(result.rows.length / pageSize)) : 1);
  let currentPage = $derived(Math.min(page, totalPages));
  let pagedRows = $derived(
    result ? result.rows.slice((currentPage - 1) * pageSize, currentPage * pageSize) : []
  );

  function goToPage(p: number) {
    page = Math.min(Math.max(p, 1), totalPages);
  }

  function setPageSize(n: number) {
    pageSize = n;
    page = 1;
  }

  // Snippets (Workbench-style saved queries)
  let isSnippetOpen = $state(false);
  let snippetName = $state('');

  function insertSnippet(sql: string) {
    if (!editorView) return;
    const sel = editorView.state.selection.main;
    editorView.dispatch({
      changes: { from: sel.from, to: sel.to, insert: sql },
      selection: { anchor: sel.from + sql.length },
      scrollIntoView: true,
    });
    editorView.focus();
    isSnippetOpen = false;
  }

  async function saveCurrentAsSnippet() {
    if (!editorView || !snippetName.trim()) return;
    await workspace.saveSnippet(snippetName.trim(), editorView.state.doc.toString());
    snippetName = '';
  }

  /** SQL completion schema: { tableName: [col1, col2, ...] } */
  function completionSchema(): Record<string, string[]> {
    const out: Record<string, string[]> = {};
    for (const [t, cols] of Object.entries(workspace.schemaCache)) {
      out[t] = cols.map((c) => c.name);
    }
    return out;
  }

  onMount(() => {
    const runKeymap = keymap.of([
      {
        key: 'Mod-Enter',
        run: () => {
          runCurrentQuery();
          return true;
        },
      },
    ]);

    editorView = new EditorView({
      doc: initialQuery,
      extensions: [
        basicSetup,
        sql({ schema: completionSchema(), upperCaseKeywords: true }),
        oneDark,
        runKeymap,
      ],
      parent: editorContainer,
    });

    return () => {
      editorView?.destroy();
    };
  });

  /** Text to execute: current selection if any, otherwise the whole doc. */
  function getExecutableText(): string {
    if (!editorView) return '';
    const sel = editorView.state.selection.main;
    if (!sel.empty) return editorView.state.sliceDoc(sel.from, sel.to);
    return editorView.state.doc.toString();
  }

  async function runCurrentQuery() {
    if (!editorView || !workspace.activeConnection) return;
    const text = getExecutableText();
    if (!text.trim()) return;

    // Production write guard: write/DDL on production-tagged connection needs confirmation.
    if (
      workspace.activeConnection.environment === 'production' &&
      isWriteStatement(text)
    ) {
      const ok = await confirmDialog.ask({
        title: t('editor.prod_guard'),
        message: t('editor.prod_confirm', {
          name: workspace.activeConnection.name,
          sql: text.slice(0, 300),
        }),
        danger: true,
      });
      if (!ok) return;
    }

    isExecuting = true;
    errorMsg = null;
    plan = null;
    try {
      result = await api.executeQuery(workspace.activeConnection.id, text);
      page = 1;
      await workspace.refreshLogs();
      await workspace.refreshTables();
      // Schema may have changed (DDL) — refresh the autocomplete cache.
      if (['CREATE', 'DROP', 'ALTER'].includes(firstVerb(text))) {
        await workspace.refreshSchemaCache();
      }
    } catch (err: any) {
      errorMsg = String(err);
      await workspace.refreshLogs();
    } finally {
      isExecuting = false;
    }
  }

  async function explainCurrentQuery() {
    if (!editorView || !workspace.activeConnection) return;
    const text = getExecutableText();
    if (!text.trim()) return;

    isExplaining = true;
    errorMsg = null;
    try {
      plan = await api.explainQuery(workspace.activeConnection.id, text);
      result = null;
      await workspace.refreshLogs();
    } catch (err: any) {
      errorMsg = String(err);
      plan = null;
    } finally {
      isExplaining = false;
    }
  }

  function exportCsv() {
    if (!result) return;
    const csvContent = 'data:text/csv;charset=utf-8,' + encodeURIComponent(rowsToCsv(result.columns, result.rows));
    const link = document.createElement('a');
    link.setAttribute('href', csvContent);
    link.setAttribute('download', 'query_result.csv');
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  }

  const planSeverity = $derived(
    plan
      ? plan.has_full_scan
        ? 'warn'
        : 'ok'
      : 'ok'
  );
</script>

{#snippet planNode(step: ExplainStep, depth: number)}
  <div class="flex items-start gap-2 py-0.5" style={`padding-left: ${depth * 16}px`}>
    <span class="text-zinc-600 shrink-0 select-none">{depth > 0 ? '↳' : '●'}</span>
    <span
      class={`font-mono ${
        step.detail.toUpperCase().includes('SCAN') && !step.detail.toUpperCase().includes('USING INDEX')
          ? 'text-amber-300'
          : step.detail.toUpperCase().includes('USING INDEX')
            ? 'text-emerald-300'
            : 'text-zinc-300'
      }`}
    >
      {step.detail}
    </span>
  </div>
  {#each step.children as child}
    {@render planNode(child, depth + 1)}
  {/each}
{/snippet}

<div class="flex-1 flex flex-col h-full bg-[#121214] text-xs select-none overflow-hidden">
  <!-- Query Action Toolbar -->
  <div class="h-10 border-b border-[#27272a] bg-[#18181b] flex items-center justify-between px-3">
    <div class="flex items-center gap-2">
      <button
        class="flex items-center gap-1.5 px-3 py-1 rounded bg-sky-600 hover:bg-sky-500 text-white font-semibold transition-colors shadow-sm disabled:opacity-50"
        onclick={runCurrentQuery}
        disabled={isExecuting}
        title={t('editor.run_hint')}
      >
        <Play class="w-3.5 h-3.5 fill-current" />
        <span>{isExecuting ? t('editor.running') : t('editor.run')}</span>
        <kbd class="ml-1 text-[10px] text-sky-200 font-mono">⌘⏎</kbd>
      </button>

      <button
        class="flex items-center gap-1.5 px-2.5 py-1 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-200 transition-colors border border-[#3f3f46]/50 disabled:opacity-50"
        onclick={explainCurrentQuery}
        disabled={isExplaining}
        title={t('editor.explain_hint')}
      >
        <Network class="w-3.5 h-3.5 text-violet-400" />
        <span>{isExplaining ? t('editor.explaining') : t('editor.explain')}</span>
      </button>

      <!-- Snippets dropdown -->
      <div class="relative">
        <button
          class="flex items-center gap-1.5 px-2.5 py-1 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-200 transition-colors border border-[#3f3f46]/50"
          onclick={() => (isSnippetOpen = !isSnippetOpen)}
          title={t('editor.snippets_hint')}
        >
          <BookMarked class="w-3.5 h-3.5 text-emerald-400" />
          <span>{t('editor.snippets')}</span>
        </button>
        {#if isSnippetOpen}
          <div class="absolute left-0 top-8 w-80 rounded-lg border border-[#27272a] bg-[#1c1c20] shadow-2xl z-40 overflow-hidden">
            <div class="max-h-56 overflow-y-auto p-1.5 space-y-0.5">
              {#if workspace.snippets.length === 0}
                <div class="p-3 text-zinc-500 italic text-[11px]">{t('editor.no_snippets')}</div>
              {:else}
                {#each workspace.snippets as snip}
                  <div class="group flex items-center gap-2 px-2.5 py-1.5 rounded hover:bg-[#27272a] cursor-pointer"
                    onclick={() => insertSnippet(snip.sql)}
                    onkeydown={(e) => e.key === 'Enter' && insertSnippet(snip.sql)}
                    tabindex="0"
                    role="button"
                    title={t('editor.snippet_insert_hint')}
                  >
                    <div class="flex-1 min-w-0">
                      <div class="text-zinc-200 font-medium truncate">{snip.name}</div>
                      <div class="text-[10px] text-zinc-500 font-mono truncate">{snip.sql}</div>
                    </div>
                    <button
                      class="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-rose-500/20 text-zinc-500 hover:text-rose-400 shrink-0"
                      onclick={(e) => { e.stopPropagation(); workspace.deleteSnippet(snip.id); }}
                      title={t('editor.snippet_delete_hint')}
                    >
                      <Trash2 class="w-3 h-3" />
                    </button>
                  </div>
                {/each}
              {/if}
            </div>
            <!-- Save current editor content -->
            <div class="border-t border-[#27272a] p-2 flex gap-1.5 bg-[#18181b]">
              <input
                type="text"
                bind:value={snippetName}
                placeholder={t('editor.snippet_save_ph')}
                onkeydown={(e) => e.key === 'Enter' && saveCurrentAsSnippet()}
                class="flex-1 bg-[#121214] border border-[#2e2e33] rounded px-2 py-1 text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-sky-500 text-[11px]"
              />
              <button
                class="px-2.5 py-1 rounded bg-sky-600/80 hover:bg-sky-500 text-white font-medium disabled:opacity-40"
                onclick={saveCurrentAsSnippet}
                disabled={!snippetName.trim()}
              >
                {t('common.save')}
              </button>
            </div>
          </div>
        {/if}
      </div>

      {#if workspace.activeConnection?.environment === 'production'}
        <span class="flex items-center gap-1 px-2 py-0.5 rounded bg-rose-500/15 border border-rose-500/40 text-rose-400 text-[10px] font-bold uppercase">
          <ShieldAlert class="w-3 h-3" /> {t('editor.prod_guard')}
        </span>
      {/if}

      {#if result}
        <div class="flex items-center gap-2 text-zinc-400 font-mono text-[11px] ml-2">
          <div class="flex items-center gap-1 text-emerald-400">
            <CheckCircle2 class="w-3.5 h-3.5" />
            <span>{t('editor.success')}</span>
          </div>
          <span>•</span>
          <div class="flex items-center gap-1">
            <Clock class="w-3 h-3 text-zinc-500" />
            <span>{result.execution_time_ms.toFixed(2)} ms</span>
          </div>
          <span>•</span>
          <span>{t('common.rows', { count: result.affected_rows })}</span>
        </div>
      {/if}
    </div>

    <!-- Export Button -->
    {#if result && result.rows.length > 0}
      <button
        class="flex items-center gap-1 px-2.5 py-1 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-200 transition-colors"
        onclick={exportCsv}
        title={t('editor.export_hint')}
      >
        <Download class="w-3.5 h-3.5 text-sky-400" />
        <span>{t('editor.export')}</span>
      </button>
    {/if}
  </div>

  <!-- CodeMirror Container (Top 40%) -->
  <div class="h-48 border-b border-[#27272a] bg-[#1e1e24] overflow-hidden" bind:this={editorContainer}></div>

  <!-- Result Area (Bottom 60%) -->
  <div class="flex-1 overflow-auto bg-[#121214] relative">
    {#if isExecuting && result}
      <!-- Re-run progress bar: keeps the previous result visible while executing -->
      <div class="absolute top-0 left-0 right-0 h-0.5 z-20 overflow-hidden">
        <div class="h-full w-1/3 bg-sky-500" style="animation: query-slide 1s ease-in-out infinite"></div>
      </div>
    {/if}
    {#if errorMsg}
      <div class="p-4 bg-rose-950/40 border-l-4 border-rose-500 text-rose-300 font-mono text-xs select-text">
        <div class="font-bold mb-1">{t('editor.exec_error')}</div>
        <div>{errorMsg}</div>
      </div>
    {:else if plan}
      <!-- Explain Plan / Query Audit view -->
      <div class="p-4 space-y-3">
        <div class="flex items-center gap-2 font-semibold text-zinc-200">
          <Network class="w-4 h-4 text-violet-400" />
          <span>{t('editor.plan_title')}</span>
          {#if planSeverity === 'warn'}
            <span class="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/15 border border-amber-500/40 text-amber-400 font-bold uppercase">{t('editor.plan_warn')}</span>
          {:else}
            <span class="text-[10px] px-1.5 py-0.5 rounded bg-emerald-500/15 border border-emerald-500/40 text-emerald-400 font-bold uppercase">{t('editor.plan_ok')}</span>
          {/if}
        </div>

        <!-- Audit warnings -->
        <div class="space-y-1.5">
          {#each plan.warnings as w}
            <div
              class={`flex items-start gap-2 p-2 rounded border text-[11px] ${
                plan.has_full_scan
                  ? 'bg-amber-950/25 border-amber-900/40 text-amber-200'
                  : 'bg-emerald-950/25 border-emerald-900/40 text-emerald-200'
              }`}
            >
              <AlertTriangle class={`w-3.5 h-3.5 mt-0.5 shrink-0 ${plan.has_full_scan ? 'text-amber-400' : 'text-emerald-400'}`} />
              <span class="select-text">{w}</span>
            </div>
          {/each}
        </div>

        <!-- Plan tree -->
        <div class="rounded-lg border border-[#27272a] bg-[#141416] p-3 font-mono text-[11px]">
          {#if plan.steps.length === 0}
            <div class="text-zinc-500 italic">{t('editor.no_plan')}</div>
          {:else}
            {#each plan.steps as step}
              {@render planNode(step, 0)}
            {/each}
          {/if}
        </div>

        <div class="flex items-center gap-3 text-[10px] text-zinc-500 font-mono">
          <span>full_scan: {plan.has_full_scan ? t('common.yes') : t('common.no')}</span>
          <span>uses_index: {plan.uses_index ? t('common.yes') : t('common.no')}</span>
          <span>temp_btree: {plan.uses_temp_btree ? t('common.yes') : t('common.no')}</span>
        </div>
      </div>
    {:else if isExecuting}
      <!-- First-run loading skeleton: shimmer rows shaped like the result grid -->
      <div class="w-full font-mono text-[11px] animate-pulse" role="status" aria-label={t('editor.running')}>
        <div class="flex border-b border-[#27272a] bg-[#18181b]">
          <div class="w-10 px-2 py-2"></div>
          {#each Array(6) as _}
            <div class="flex-1 px-3 py-2"><div class="h-2.5 w-2/3 rounded bg-[#27272a]"></div></div>
          {/each}
        </div>
        {#each Array(14) as _, r}
          <div class="flex border-b border-[#1f1f23]">
            <div class="w-10 px-2 py-2"></div>
            {#each Array(6) as _, c}
              <div class="flex-1 px-3 py-2">
                <div
                  class={`h-2.5 rounded ${r % 2 === 0 ? 'bg-[#1a1a1e]' : 'bg-[#1e1e22]'}`}
                  style={`width: ${45 + ((r * 7 + c * 13) % 45)}%`}
                ></div>
              </div>
            {/each}
          </div>
        {/each}
      </div>
    {:else if !result}
      <div class="p-8 text-center text-zinc-600 font-mono text-xs">
        {t('editor.hint_run')}
        <div class="mt-2 text-[10px]">{t('editor.hint_select')}</div>
      </div>
    {:else}
      <table class="w-full text-left border-collapse font-mono text-[11px]">
        <thead class="sticky top-0 bg-[#18181b] border-b border-[#27272a] text-zinc-400 z-10">
          <tr>
            <th class="w-10 px-2 py-1.5 border-r border-[#27272a] text-center text-zinc-600">#</th>
            {#each result.columns as col}
              <th class="px-3 py-1.5 border-r border-[#27272a] text-zinc-200 font-semibold truncate">{col}</th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each pagedRows as row, idx (idx)}
            <tr class="border-b border-[#1f1f23] hover:bg-[#1a1a1e]">
              <td class="px-2 py-1.5 border-r border-[#27272a] text-center text-zinc-600 text-[10px]">{(currentPage - 1) * pageSize + idx + 1}</td>
              {#each row as cell}
                <td class="px-3 py-1.5 border-r border-[#27272a] text-zinc-300 truncate max-w-xs select-text">
                  {#if cell === null}
                    <span class="text-zinc-600 italic">NULL</span>
                  {:else}
                    {cell}
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <!-- Status bar: pagination -->
  {#if result && !plan}
    <div class="h-8 shrink-0 border-t border-[#27272a] bg-[#18181b] flex items-center justify-between px-3 text-[10px] font-mono text-zinc-500">
      <div class="flex items-center gap-3">
        <span class="text-zinc-400">{t('common.rows', { count: result.rows.length.toLocaleString() })}</span>
        {#if isExecuting}
          <span class="text-sky-400 flex items-center gap-1">
            <span class="w-2.5 h-2.5 border border-sky-500 border-t-transparent rounded-full animate-spin inline-block"></span>
            {t('editor.running')}
          </span>
        {/if}
      </div>

      <div class="flex items-center gap-1">
        <select
          class="bg-[#121214] border border-[#2e2e33] rounded px-1.5 py-0.5 text-zinc-300 focus:outline-none focus:border-sky-500"
          title={t('grid.rows_per_page')}
          value={pageSize}
          onchange={(e) => setPageSize(Number((e.target as HTMLSelectElement).value))}
        >
          {#each [50, 100, 250, 500, 1000] as n}
            <option value={n}>{n}</option>
          {/each}
        </select>

        <button
          class="p-1 rounded hover:bg-[#27272a] disabled:opacity-30"
          disabled={currentPage <= 1 || isExecuting}
          onclick={() => goToPage(1)}
          title={t('grid.first_page')}
        >
          <ChevronsLeft class="w-3.5 h-3.5" />
        </button>
        <button
          class="p-1 rounded hover:bg-[#27272a] disabled:opacity-30"
          disabled={currentPage <= 1 || isExecuting}
          onclick={() => goToPage(currentPage - 1)}
          title={t('grid.prev_page')}
        >
          <ChevronLeft class="w-3.5 h-3.5" />
        </button>
        <span class="px-1 text-zinc-400">{t('grid.page_of', { page: currentPage, total: totalPages })}</span>
        <button
          class="p-1 rounded hover:bg-[#27272a] disabled:opacity-30"
          disabled={currentPage >= totalPages || isExecuting}
          onclick={() => goToPage(currentPage + 1)}
          title={t('grid.next_page')}
        >
          <ChevronRight class="w-3.5 h-3.5" />
        </button>
        <button
          class="p-1 rounded hover:bg-[#27272a] disabled:opacity-30"
          disabled={currentPage >= totalPages || isExecuting}
          onclick={() => goToPage(totalPages)}
          title={t('grid.last_page')}
        >
          <ChevronsRight class="w-3.5 h-3.5" />
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  @keyframes query-slide {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(300%);
    }
  }
</style>
