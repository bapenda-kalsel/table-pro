<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import type { ExplainResult, ExplainStep, QueryResultData } from '$lib/types';
  import { Play, Download, Clock, CheckCircle2, Network, AlertTriangle, ShieldAlert, BookMarked, Trash2, ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight, Sparkles, Settings, X } from 'lucide-svelte';
  import { firstVerb, rowsToCsv } from '$lib/utils/sql';
  import { withProdGuard } from '$lib/api/approval';
  import { t } from '$lib/i18n/i18n.svelte';
  import { onMount } from 'svelte';
  import { EditorView, basicSetup } from 'codemirror';
  import { sql } from '@codemirror/lang-sql';
  import { oneDark } from '@codemirror/theme-one-dark';
  import { theme } from '$lib/stores/theme.svelte';
  import { keymap } from '@codemirror/view';

  import type { WorkspaceTab } from '$lib/types';

  let { tab }: { tab: WorkspaceTab } = $props();

  let editorContainer: HTMLDivElement;
  let editorView: EditorView | null = null;
  let isExecuting = $state(false);
  let isExplaining = $state(false);
  let result = $state<QueryResultData | null>(null);
  let plan = $state<ExplainResult | null>(null);
  let errorMsg = $state<string | null>(null);

  // AI assistant
  let aiPrompt = $state('');
  let aiBusy = $state(false);
  let aiResult = $state<string | null>(null);
  let aiError = $state<string | null>(null);

  async function generateWithAi() {
    const connId = workspace.activeConnection?.id;
    if (!connId || !aiPrompt.trim() || aiBusy) return;
    aiBusy = true;
    aiError = null;
    try {
      const res = await api.aiGenerate(connId, aiPrompt.trim(), getExecutableText());
      aiResult = res.sql;
    } catch (err: any) {
      aiError = String(err);
      aiResult = null;
    } finally {
      aiBusy = false;
    }
  }

  /** Insert generated SQL at the cursor, or replace the whole document. */
  function applyAiSql(mode: 'insert' | 'replace') {
    if (!editorView || !aiResult) return;
    if (mode === 'replace') {
      editorView.dispatch({ changes: { from: 0, to: editorView.state.doc.length, insert: aiResult } });
    } else {
      const sel = editorView.state.selection.main;
      editorView.dispatch({ changes: { from: sel.from, to: sel.to, insert: aiResult } });
    }
    aiResult = null;
    aiPrompt = '';
    editorView.focus();
  }

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
      doc: tab.queryText ?? '',
      extensions: [
        basicSetup,
        sql({ schema: completionSchema(), upperCaseKeywords: true }),
        ...(theme.current === 'dark' ? [oneDark] : []),
        runKeymap,
        // Keep the draft in tab state so workspace-level unsaved-work
        // checks (switch/disconnect/close) see edits made here.
        EditorView.updateListener.of((u) => {
          if (u.docChanged) {
            workspace.updateTabDraft(tab.id, u.state.doc.toString());
          }
        }),
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

    // Production writes are confirmed through the backend challenge — the
    // server analyzes the whole batch and issues a one-shot approval token
    // bound to this exact statement.
    isExecuting = true;
    errorMsg = null;
    plan = null;
    try {
      result = await withProdGuard((tok) =>
        api.executeQuery(workspace.activeConnection!.id, text, tok)
      );
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
    <span class="text-faint shrink-0 select-none">{depth > 0 ? '↳' : '●'}</span>
    <span
      class={`font-mono ${
        step.detail.toUpperCase().includes('SCAN') && !step.detail.toUpperCase().includes('USING INDEX')
          ? 'text-warn'
          : step.detail.toUpperCase().includes('USING INDEX')
            ? 'text-ok'
            : 'text-fg2'
      }`}
    >
      {step.detail}
    </span>
  </div>
  {#each step.children as child}
    {@render planNode(child, depth + 1)}
  {/each}
{/snippet}

<div class="flex-1 flex flex-col h-full bg-app text-xs select-none overflow-hidden">
  <!-- Query Action Toolbar -->
  <div class="h-10 border-b border-edge bg-panel flex items-center justify-between px-3">
    <div class="flex items-center gap-2">
      <button
        class="flex items-center gap-1.5 px-3 py-1 rounded-md bg-accent-strong hover:opacity-95 text-white font-semibold transition-all disabled:opacity-50"
        onclick={runCurrentQuery}
        disabled={isExecuting}
        title={t('editor.run_hint')}
      >
        <Play class="w-3.5 h-3.5 fill-current" />
        <span>{isExecuting ? t('editor.running') : t('editor.run')}</span>
        <kbd class="ml-1 text-[10px] opacity-85 font-mono">⌘⏎</kbd>
      </button>

      <button
        class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-elevated hover:bg-hover text-fg transition-colors border border-edge disabled:opacity-50"
        onclick={explainCurrentQuery}
        disabled={isExplaining}
        title={t('editor.explain_hint')}
      >
        <Network class="w-3.5 h-3.5 text-muted" />
        <span>{isExplaining ? t('editor.explaining') : t('editor.explain')}</span>
      </button>

      <!-- Snippets dropdown -->
      <div class="relative">
        <button
          class="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-elevated hover:bg-hover text-fg transition-colors border border-edge"
          onclick={() => (isSnippetOpen = !isSnippetOpen)}
          title={t('editor.snippets_hint')}
        >
          <BookMarked class="w-3.5 h-3.5 text-muted" />
          <span>{t('editor.snippets')}</span>
        </button>
        {#if isSnippetOpen}
          <div class="absolute left-0 top-8 w-80 rounded-lg border border-edge bg-elevated shadow-2xl z-40 overflow-hidden">
            <div class="max-h-56 overflow-y-auto p-1.5 space-y-0.5">
              {#if workspace.snippets.length === 0}
                <div class="p-3 text-faint text-[11px]">{t('editor.no_snippets')}</div>
              {:else}
                {#each workspace.snippets as snip}
                  <div class="group flex items-center gap-2 px-2.5 py-1.5 rounded hover:bg-hover cursor-pointer"
                    onclick={() => insertSnippet(snip.sql)}
                    onkeydown={(e) => e.key === 'Enter' && insertSnippet(snip.sql)}
                    tabindex="0"
                    role="button"
                    title={t('editor.snippet_insert_hint')}
                  >
                    <div class="flex-1 min-w-0">
                      <div class="text-fg font-medium truncate">{snip.name}</div>
                      <div class="text-[10px] text-faint font-mono truncate">{snip.sql}</div>
                    </div>
                    <button
                      class="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-err-bg text-faint hover:text-err shrink-0"
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
            <div class="border-t border-edge p-2 flex gap-1.5 bg-panel">
              <input
                type="text"
                bind:value={snippetName}
                placeholder={t('editor.snippet_save_ph')}
                onkeydown={(e) => e.key === 'Enter' && saveCurrentAsSnippet()}
                class="flex-1 bg-input border border-edge rounded px-2 py-1 text-fg placeholder-faint focus:outline-none focus:border-accent text-[11px]"
              />
              <button
                class="px-2.5 py-1 rounded bg-accent-strong hover:opacity-95 text-white font-medium disabled:opacity-40"
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
        <span class="flex items-center gap-1 px-2 py-0.5 rounded bg-err-bg border border-err-edge text-err text-[10px] font-mono font-bold uppercase">
          <ShieldAlert class="w-3 h-3" /> {t('editor.prod_guard')}
        </span>
      {/if}

      {#if result}
        <div class="flex items-center gap-2 text-muted font-mono text-[11px] ml-2">
          <div class="flex items-center gap-1 text-ok">
            <CheckCircle2 class="w-3.5 h-3.5" />
            <span>{t('editor.success')}</span>
          </div>
          <span class="text-faint">/</span>
          <div class="flex items-center gap-1">
            <Clock class="w-3 h-3 text-faint" />
            <span>{result.execution_time_ms.toFixed(2)} ms</span>
          </div>
          <span class="text-faint">/</span>
          <span>{t('common.rows', { count: result.affected_rows })}</span>
        </div>
      {/if}
    </div>

    <!-- Export Button -->
    {#if result && result.rows.length > 0}
      <button
        class="flex items-center gap-1 px-2.5 py-1 rounded-md bg-elevated hover:bg-hover border border-edge text-fg transition-colors"
        onclick={exportCsv}
        title={t('editor.export_hint')}
      >
        <Download class="w-3.5 h-3.5 text-muted" />
        <span>{t('editor.export')}</span>
      </button>
    {/if}
  </div>

  <!-- AI Assistant bar -->
  <div class="border-b border-edge bg-panel px-3 py-1.5 flex items-center gap-2">
    <Sparkles class="w-3.5 h-3.5 text-accent shrink-0" />
    <input
      type="text"
      bind:value={aiPrompt}
      placeholder={t('ai.prompt_placeholder')}
      onkeydown={(e) => e.key === 'Enter' && generateWithAi()}
      class="flex-1 bg-transparent text-fg placeholder-faint focus:outline-none text-[11px] font-mono"
    />
    <button
      class="px-2.5 py-0.5 rounded bg-accent-soft hover:opacity-90 border border-accent-edge text-accent text-[11px] font-medium transition-colors disabled:opacity-40"
      onclick={generateWithAi}
      disabled={aiBusy || !aiPrompt.trim() || !workspace.activeConnection}
    >
      {aiBusy ? t('ai.generating') : t('ai.generate')}
    </button>
    <button
      class="p-1 rounded hover:bg-hover text-faint hover:text-fg transition-colors"
      onclick={() => (workspace.isAiSettingsOpen = true)}
      title={t('ai.settings_hint')}
    >
      <Settings class="w-3.5 h-3.5" />
    </button>
  </div>

  {#if aiError || aiResult}
    <div class="border-b border-edge bg-subtle px-3 py-2 space-y-1.5">
      {#if aiError}
        <div class="text-err text-[11px]">{aiError}</div>
      {/if}
      {#if aiResult}
        <pre class="font-mono text-[11px] text-fg whitespace-pre-wrap select-text">{aiResult}</pre>
        <div class="flex items-center gap-2">
          <button
            class="px-2.5 py-0.5 rounded bg-accent-strong hover:opacity-95 text-white text-[11px] font-medium"
            onclick={() => applyAiSql('insert')}
          >{t('ai.insert')}</button>
          <button
            class="px-2.5 py-0.5 rounded bg-elevated hover:bg-hover text-fg text-[11px] border border-edge"
            onclick={() => applyAiSql('replace')}
          >{t('ai.replace')}</button>
          <span class="flex-1"></span>
          <span class="text-[10px] text-faint">{t('ai.run_hint')}</span>
          <button class="p-0.5 text-faint hover:text-fg" onclick={() => (aiResult = null)} title={t('ai.dismiss')}>
            <X class="w-3 h-3" />
          </button>
        </div>
      {/if}
    </div>
  {/if}

  <!-- CodeMirror Container (Top 40%) -->
  <div class="h-48 border-b border-edge bg-elevated overflow-hidden" bind:this={editorContainer}></div>

  <!-- Result Area (Bottom 60%) -->
  <div class="flex-1 overflow-auto bg-app relative">
    {#if isExecuting && result}
      <!-- Re-run progress bar: keeps the previous result visible while executing -->
      <div class="absolute top-0 left-0 right-0 h-0.5 z-20 overflow-hidden">
        <div class="h-full w-1/3 bg-accent" style="animation: query-slide 1s ease-in-out infinite"></div>
      </div>
    {/if}
    {#if errorMsg}
      <div class="p-4 bg-err-bg border-l-2 border-err text-err font-mono text-xs select-text">
        <div class="font-bold mb-1">{t('editor.exec_error')}</div>
        <div>{errorMsg}</div>
      </div>
    {:else if plan}
      <!-- Explain Plan / Query Audit view -->
      <div class="p-4 space-y-3">
        <div class="flex items-center gap-2 font-semibold text-fg">
          <Network class="w-4 h-4 text-accent" />
          <span>{t('editor.plan_title')}</span>
          {#if planSeverity === 'warn'}
            <span class="text-[10px] px-1.5 py-0.5 rounded bg-warn-bg border border-warn-edge text-warn font-mono font-bold uppercase">{t('editor.plan_warn')}</span>
          {:else}
            <span class="text-[10px] px-1.5 py-0.5 rounded bg-ok-bg border border-ok-edge text-ok font-mono font-bold uppercase">{t('editor.plan_ok')}</span>
          {/if}
        </div>

        <!-- Audit warnings -->
        <div class="space-y-1.5">
          {#each plan.warnings as w}
            <div
              class={`flex items-start gap-2 p-2.5 rounded-md border text-[11px] ${
                plan.has_full_scan
                  ? 'bg-warn-bg border-warn-edge text-warn'
                  : 'bg-ok-bg border-ok-edge text-ok'
              }`}
            >
              <AlertTriangle class={`w-3.5 h-3.5 mt-0.5 shrink-0 ${plan.has_full_scan ? 'text-warn' : 'text-ok'}`} />
              <span class="select-text">{w}</span>
            </div>
          {/each}
        </div>

        <!-- Plan tree -->
        <div class="rounded-lg border border-edge bg-subtle p-3 font-mono text-[11px]">
          {#if plan.steps.length === 0}
            <div class="text-faint">{t('editor.no_plan')}</div>
          {:else}
            {#each plan.steps as step}
              {@render planNode(step, 0)}
            {/each}
          {/if}
        </div>

        <div class="flex items-center gap-3 text-[10px] text-faint font-mono">
          <span>full_scan: {plan.has_full_scan ? t('common.yes') : t('common.no')}</span>
          <span>uses_index: {plan.uses_index ? t('common.yes') : t('common.no')}</span>
          <span>temp_btree: {plan.uses_temp_btree ? t('common.yes') : t('common.no')}</span>
        </div>
      </div>
    {:else if isExecuting}
      <!-- First-run loading skeleton: shimmer rows shaped like the result grid -->
      <div class="w-full font-mono text-[11px] animate-pulse" role="status" aria-label={t('editor.running')}>
        <div class="flex border-b border-edge bg-panel">
          <div class="w-10 px-2 py-2"></div>
          {#each Array(6) as _}
            <div class="flex-1 px-3 py-2"><div class="h-2.5 w-2/3 rounded bg-elevated"></div></div>
          {/each}
        </div>
        {#each Array(14) as _, r}
          <div class="flex border-b border-edge">
            <div class="w-10 px-2 py-2"></div>
            {#each Array(6) as _, c}
              <div class="flex-1 px-3 py-2">
                <div
                  class={`h-2.5 rounded ${r % 2 === 0 ? 'bg-subtle' : 'bg-elevated'}`}
                  style={`width: ${45 + ((r * 7 + c * 13) % 45)}%`}
                ></div>
              </div>
            {/each}
          </div>
        {/each}
      </div>
    {:else if !result}
      <div class="p-10 flex flex-col items-center justify-center text-center gap-2.5 text-faint">
        <div class="w-9 h-9 rounded-lg bg-elevated border border-edge flex items-center justify-center text-muted">
          <Play class="w-4 h-4" />
        </div>
        <div class="font-medium text-fg2 text-xs">{t('editor.hint_run')}</div>
        <div class="text-[11px] text-muted max-w-md">{t('editor.hint_select')}</div>
      </div>
    {:else}
      <table class="w-full text-left border-collapse font-mono text-[11px]">
        <thead class="sticky top-0 bg-panel border-b border-edge text-muted z-10">
          <tr>
            <th class="w-10 px-2 py-1.5 border-r border-edge text-center text-faint">#</th>
            {#each result.columns as col}
              <th class="px-3 py-1.5 border-r border-edge text-fg font-semibold truncate">{col}</th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each pagedRows as row, idx (idx)}
            <tr class="border-b border-edge hover:bg-hover">
              <td class="px-2 py-1.5 border-r border-edge text-center text-faint text-[10px]">{(currentPage - 1) * pageSize + idx + 1}</td>
              {#each row as cell}
                <td class="px-3 py-1.5 border-r border-edge text-fg2 truncate max-w-xs select-text">
                  {#if cell === null}
                    <span class="text-faint">NULL</span>
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
    <div class="h-8 shrink-0 border-t border-edge bg-panel flex items-center justify-between px-3 text-[10px] font-mono text-faint">
      <div class="flex items-center gap-3">
        <span class="text-muted">{t('common.rows', { count: result.rows.length.toLocaleString() })}</span>
        {#if isExecuting}
          <span class="text-accent flex items-center gap-1.5">
            <span class="w-1.5 h-1.5 rounded-full bg-accent animate-pulse inline-block"></span>
            {t('editor.running')}
          </span>
        {/if}
      </div>

      <div class="flex items-center gap-1">
        <select
          class="bg-app border border-edge rounded px-1.5 py-0.5 text-fg2 focus:outline-none focus:border-accent"
          title={t('grid.rows_per_page')}
          value={pageSize}
          onchange={(e) => setPageSize(Number((e.target as HTMLSelectElement).value))}
        >
          {#each [50, 100, 250, 500, 1000] as n}
            <option value={n}>{n}</option>
          {/each}
        </select>

        <button
          class="p-1 rounded hover:bg-hover disabled:opacity-30"
          disabled={currentPage <= 1 || isExecuting}
          onclick={() => goToPage(1)}
          title={t('grid.first_page')}
        >
          <ChevronsLeft class="w-3.5 h-3.5" />
        </button>
        <button
          class="p-1 rounded hover:bg-hover disabled:opacity-30"
          disabled={currentPage <= 1 || isExecuting}
          onclick={() => goToPage(currentPage - 1)}
          title={t('grid.prev_page')}
        >
          <ChevronLeft class="w-3.5 h-3.5" />
        </button>
        <span class="px-1 text-muted">{t('grid.page_of', { page: currentPage, total: totalPages })}</span>
        <button
          class="p-1 rounded hover:bg-hover disabled:opacity-30"
          disabled={currentPage >= totalPages || isExecuting}
          onclick={() => goToPage(currentPage + 1)}
          title={t('grid.next_page')}
        >
          <ChevronRight class="w-3.5 h-3.5" />
        </button>
        <button
          class="p-1 rounded hover:bg-hover disabled:opacity-30"
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
