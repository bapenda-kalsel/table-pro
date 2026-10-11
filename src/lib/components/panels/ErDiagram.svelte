<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import type { FullTableSchema } from '$lib/types';
  import { Network, RefreshCw, ZoomIn, ZoomOut, Maximize, KeyRound, Link, Table, Eye, Link2, X, AlertTriangle, Check } from 'lucide-svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { buildAddFkSql, FK_ACTIONS } from '$lib/utils/sql';

  const NODE_W = 230;
  const HEADER_H = 30;
  const ROW_H = 19;
  const PAD = 8;

  let schemas = $state<Record<string, FullTableSchema>>({});
  let loading = $state(true);
  let zoom = $state(1.0);
  let drag = $state<{ table: string; dx: number; dy: number } | null>(null);
  let hoveredEdge = $state<number | null>(null);

  // --- Visual relation builder (phpMyAdmin Designer style) ---
  let linkMode = $state(false);
  let linkSource = $state<{ table: string; column: string } | null>(null);
  let fkTarget = $state<{ table: string; column: string } | null>(null);
  let fkName = $state('');
  let fkOnDelete = $state('NO ACTION');
  let fkOnUpdate = $state('NO ACTION');
  let fkBusy = $state(false);
  let fkError = $state<string | null>(null);
  let fkToast = $state<string | null>(null);

  function toggleLinkMode() {
    linkMode = !linkMode;
    linkSource = null;
  }

  function onColumnClick(table: string, column: string) {
    if (!linkMode) return;
    if (!linkSource) {
      linkSource = { table, column };
    } else {
      fkTarget = { table, column };
      fkName = `fk_${linkSource.table}_${linkSource.column}`;
      fkOnDelete = 'NO ACTION';
      fkOnUpdate = 'NO ACTION';
      fkError = null;
    }
  }

  function closeFkModal() {
    fkTarget = null;
    fkError = null;
  }

  function cancelLink() {
    linkMode = false;
    linkSource = null;
    fkTarget = null;
  }

  let fkPreviewSql = $derived(
    linkSource && fkTarget
      ? buildAddFkSql(
          fkName.trim() || `fk_${linkSource.table}_${linkSource.column}`,
          linkSource.table,
          linkSource.column,
          fkTarget.table,
          fkTarget.column,
          fkOnDelete,
          fkOnUpdate,
        )
      : '',
  );

  async function createFk() {
    if (!linkSource || !fkTarget || !workspace.activeConnection) return;
    fkBusy = true;
    fkError = null;
    try {
      const msg = await api.addForeignKey({
        connectionId: workspace.activeConnection.id,
        constraintName: fkName.trim() || `fk_${linkSource.table}_${linkSource.column}`,
        table: linkSource.table,
        column: linkSource.column,
        refTable: fkTarget.table,
        refColumn: fkTarget.column,
        onDelete: fkOnDelete,
        onUpdate: fkOnUpdate,
      });
      fkToast = msg;
      setTimeout(() => (fkToast = null), 8000);
      cancelLink();
      await load();
      workspace.refreshSchemaCache();
    } catch (err: any) {
      fkError = String(err);
    } finally {
      fkBusy = false;
    }
  }

  interface Edge {
    fromTable: string;
    fromCol: string;
    toTable: string;
    toCol: string;
  }

  let edges = $derived.by<Edge[]>(() => {
    const out: Edge[] = [];
    for (const [t, s] of Object.entries(schemas)) {
      for (const fk of s.foreign_keys) {
        if (schemas[fk.referenced_table]) {
          out.push({
            fromTable: t,
            fromCol: fk.from_column,
            toTable: fk.referenced_table,
            toCol: fk.to_column ?? '',
          });
        }
      }
    }
    return out;
  });

  async function load() {
    if (!workspace.activeConnection) return;
    loading = true;
    const map: Record<string, FullTableSchema> = {};
    for (const t of workspace.tables) {
      try {
        map[t.name] = await api.getFullSchema(workspace.activeConnection.id, t.name);
      } catch {
        // skip unreadable objects
      }
    }
    schemas = map;
    // default layout: grid, for tables without a stored position
    const names = Object.keys(map);
    const cols = Math.max(1, Math.ceil(Math.sqrt(names.length)));
    names.forEach((name, i) => {
      if (!workspace.erdPositions[name]) {
        const row = Math.floor(i / cols);
        const col = i % cols;
        workspace.erdPositions[name] = { x: 60 + col * (NODE_W + 60), y: 50 + row * 230 };
      }
    });
    loading = false;
  }

  $effect(() => {
    // reload when connection/table list changes
    if (workspace.activeConnection && workspace.tables) {
      load();
    }
  });

  function nodeHeight(s: FullTableSchema): number {
    return HEADER_H + s.columns.length * ROW_H + PAD;
  }

  function colRowY(table: string, colName: string): number {
    const s = schemas[table];
    const pos = workspace.erdPositions[table] ?? { x: 0, y: 0 };
    const idx = s ? s.columns.findIndex((c) => c.name === colName) : -1;
    const i = idx === -1 ? 0 : idx;
    return pos.y + HEADER_H + i * ROW_H + ROW_H / 2;
  }

  function edgePath(e: Edge): string {
    const fs = workspace.erdPositions[e.fromTable] ?? { x: 0, y: 0 };
    const ts = workspace.erdPositions[e.toTable] ?? { x: 0, y: 0 };
    const sy = colRowY(e.fromTable, e.fromCol);
    const ty = colRowY(e.toTable, e.toCol);
    // FK leaf (many side) -> PK parent (one side)
    const sx = fs.x + NODE_W;
    const tx = ts.x;
    const dx = Math.max(40, Math.abs(tx - sx) / 2);
    return `M ${sx} ${sy} C ${sx + dx} ${sy}, ${tx - dx} ${ty}, ${tx} ${ty}`;
  }

  function startDrag(e: PointerEvent, table: string) {
    const pos = workspace.erdPositions[table];
    if (!pos) return;
    drag = { table, dx: e.clientX / zoom - pos.x, dy: e.clientY / zoom - pos.y };
  }

  function onPointerMove(e: PointerEvent) {
    if (!drag) return;
    workspace.erdPositions[drag.table] = {
      x: Math.max(0, e.clientX / zoom - drag.dx),
      y: Math.max(0, e.clientY / zoom - drag.dy),
    };
  }

  function endDrag() {
    drag = null;
  }

  function autoLayout() {
    workspace.erdPositions = {};
    load();
  }

  // canvas size = furthest node + margin
  let canvasSize = $derived.by(() => {
    let w = 900;
    let h = 600;
    for (const [name, pos] of Object.entries(workspace.erdPositions)) {
      const s = schemas[name];
      w = Math.max(w, pos.x + NODE_W + 120);
      h = Math.max(h, pos.y + (s ? nodeHeight(s) : 60) + 120);
    }
    return { w, h };
  });
</script>

{#snippet tableIcon(type?: string)}
  {#if type === 'view'}
    <Eye class="w-3.5 h-3.5 text-indigo-400 shrink-0" />
  {:else}
    <Table class="w-3.5 h-3.5 text-sky-400 shrink-0" />
  {/if}
{/snippet}

<svelte:window onpointermove={onPointerMove} onpointerup={endDrag} />

<div class="flex-1 flex flex-col h-full bg-[#0f0f12] text-xs select-none overflow-hidden">
  <!-- Toolbar -->
  <div class="h-10 border-b border-[#27272a] bg-[#18181b] flex items-center justify-between px-4">
    <div class="flex items-center gap-2 font-semibold text-zinc-200">
      <Network class="w-4 h-4 text-violet-400" />
      <span>{t('erd.title')}</span>
      <span class="text-[10px] text-zinc-500 font-mono">
        {t('erd.summary', { tables: Object.keys(schemas).length, rels: edges.length })}
      </span>
    </div>
    <div class="flex items-center gap-1">
      <button
        class={`flex items-center gap-1.5 px-2 py-1 rounded text-[10px] font-semibold transition-colors ${
          linkMode
            ? 'bg-violet-500/20 border border-violet-500/50 text-violet-300'
            : 'hover:bg-[#27272a] text-zinc-400 border border-transparent'
        }`}
        onclick={toggleLinkMode}
        title={t('erd.new_relation')}
      >
        <Link2 class="w-3.5 h-3.5" />
        <span>{t('erd.new_relation')}</span>
      </button>
      <div class="w-px h-4 bg-[#3f3f46] mx-1"></div>
      <button class="p-1.5 rounded hover:bg-[#27272a] text-zinc-400" onclick={() => (zoom = Math.max(0.4, zoom - 0.15))} title={t('erd.zoom_out')}>
        <ZoomOut class="w-3.5 h-3.5" />
      </button>
      <span class="w-10 text-center text-[10px] font-mono text-zinc-400">{Math.round(zoom * 100)}%</span>
      <button class="p-1.5 rounded hover:bg-[#27272a] text-zinc-400" onclick={() => (zoom = Math.min(2, zoom + 0.15))} title={t('erd.zoom_in')}>
        <ZoomIn class="w-3.5 h-3.5" />
      </button>
      <button class="p-1.5 rounded hover:bg-[#27272a] text-zinc-400 ml-1" onclick={autoLayout} title={t('erd.autolayout')}>
        <Maximize class="w-3.5 h-3.5" />
      </button>
      <button class="p-1.5 rounded hover:bg-[#27272a] text-zinc-400" onclick={load} title={t('erd.reload')}>
        <RefreshCw class="w-3.5 h-3.5" />
      </button>
    </div>
  </div>

  <!-- Link-mode banner -->
  {#if linkMode}
    <div class="h-7 border-b border-violet-500/30 bg-violet-500/10 flex items-center gap-2 px-3 text-[10px] text-violet-300">
      <Link2 class="w-3 h-3" />
      <span>{linkSource ? t('erd.link_pick_second') : t('erd.link_pick_first')}</span>
      {#if linkSource}
        <span class="font-mono text-violet-200">{linkSource.table}.{linkSource.column}</span>
      {/if}
      <button class="ml-auto flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-violet-500/20" onclick={cancelLink}>
        <X class="w-3 h-3" /> {t('common.cancel')}
      </button>
    </div>
  {/if}

  <!-- FK created toast -->
  {#if fkToast}
    <div class="border-b border-emerald-500/40 bg-emerald-500/10 px-3 py-1.5 text-[10px] text-emerald-300 flex items-center gap-2">
      <Check class="w-3 h-3" /> <span class="truncate font-mono">{fkToast}</span>
    </div>
  {/if}

  <!-- Canvas -->
  <div class="flex-1 overflow-auto relative" style="background-image: radial-gradient(circle, #26262c 1px, transparent 1px); background-size: 24px 24px;">
    {#if loading}
      <div class="flex items-center justify-center h-48 text-zinc-500 gap-2">
        <div class="w-4 h-4 border-2 border-violet-500 border-t-transparent rounded-full animate-spin"></div>
        <span>{t('erd.loading')}</span>
      </div>
    {:else if Object.keys(schemas).length === 0}
      <div class="p-8 text-center text-zinc-500">{t('erd.empty')}</div>
    {:else}
      <div
        class="relative origin-top-left"
        style={`width: ${canvasSize.w}px; height: ${canvasSize.h}px; transform: scale(${zoom});`}
      >
        <!-- Edges (SVG layer beneath nodes) -->
        <svg class="absolute inset-0 pointer-events-none" width={canvasSize.w} height={canvasSize.h}>
          {#each edges as e, i}
            <path
              d={edgePath(e)}
              fill="none"
              stroke={hoveredEdge === i ? '#a78bfa' : '#52525b'}
              stroke-width={hoveredEdge === i ? 2 : 1.2}
              class="pointer-events-auto cursor-help transition-all"
              onmouseenter={() => (hoveredEdge = i)}
              onmouseleave={() => (hoveredEdge = null)}
            >
              <title>{e.fromTable}.{e.fromCol} → {e.toTable}.{e.toCol}</title>
            </path>
            <!-- crow's-foot-ish marker on the "one" side -->
            {@const pos = workspace.erdPositions[e.toTable] ?? { x: 0, y: 0 }}
            <circle cx={pos.x} cy={colRowY(e.toTable, e.toCol)} r="3" fill={hoveredEdge === i ? '#a78bfa' : '#71717a'} />
          {/each}
        </svg>

        <!-- Table nodes -->
        {#each Object.entries(schemas) as [name, s]}
          {@const pos = workspace.erdPositions[name] ?? { x: 40, y: 40 }}
          <div
            class={`absolute rounded-lg border bg-[#18181b] shadow-xl overflow-hidden ${
              drag?.table === name ? 'border-violet-500 shadow-violet-500/20 z-20' : 'border-[#3f3f46] z-10'
            }`}
            style={`left: ${pos.x}px; top: ${pos.y}px; width: ${NODE_W}px;`}
          >
            <!-- Node header (drag handle) -->
            <div
              class="h-[30px] px-2.5 flex items-center gap-2 bg-[#222226] border-b border-[#3f3f46] cursor-grab active:cursor-grabbing"
              onpointerdown={(e) => startDrag(e, name)}
              role="button"
              tabindex="0"
            >
              {@render tableIcon(workspace.tables.find((t) => t.name === name)?.table_type)}
              <span class="font-bold text-zinc-100 truncate text-[11px]">{name}</span>
              <span class="ml-auto text-[9px] text-zinc-500 font-mono">{t('erd.cols', { count: s.columns.length })}</span>
            </div>
            <!-- Columns -->
            <div class="font-mono text-[10px]">
              {#each s.columns as c}
                <div
                  class={`h-[19px] px-2.5 flex items-center gap-1.5 border-b border-[#27272a]/40 text-zinc-300 truncate ${
                    linkMode ? 'cursor-crosshair hover:bg-violet-500/15' : ''
                  } ${linkSource?.table === name && linkSource?.column === c.name ? 'bg-violet-500/25 text-violet-100' : ''}`}
                  role={linkMode ? 'button' : undefined}
                  onclick={() => onColumnClick(name, c.name)}
                >
                  {#if c.is_primary_key}
                    <KeyRound class="w-3 h-3 text-amber-400 shrink-0" />
                  {:else if s.foreign_keys.some((f) => f.from_column === c.name)}
                    <Link class="w-3 h-3 text-violet-400 shrink-0" />
                  {:else}
                    <span class="w-3 shrink-0"></span>
                  {/if}
                  <span class={`truncate ${c.is_primary_key ? 'text-amber-200 font-semibold' : ''}`}>{c.name}</span>
                  <span class="ml-auto text-zinc-500 text-[9px] shrink-0">{c.data_type || 'ANY'}</span>
                </div>
              {/each}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Legend -->
  <div class="h-7 border-t border-[#27272a] bg-[#141416] flex items-center gap-4 px-3 text-[10px] text-zinc-500">
    <span class="flex items-center gap-1"><KeyRound class="w-3 h-3 text-amber-400" /> {t('erd.legend_pk')}</span>
    <span class="flex items-center gap-1"><Link class="w-3 h-3 text-violet-400" /> {t('erd.legend_fk')}</span>
    <span class="flex items-center gap-1"><span class="inline-block w-4 h-px bg-zinc-500"></span> {t('erd.legend_rel')}</span>
    <span class="ml-auto">{t('erd.legend_hint')}</span>
  </div>

  <!-- FK creation modal -->
  {#if fkTarget && linkSource}
    <div class="fixed inset-0 bg-black/60 backdrop-blur-[2px] flex items-center justify-center z-50 p-4">
      <div class="bg-[#18181b] border border-[#3f3f46] rounded-xl w-full max-w-md shadow-2xl overflow-hidden">
        <div class="px-4 py-3 border-b border-[#27272a] bg-[#1f1f23] flex items-center justify-between">
          <div class="flex items-center gap-2 text-zinc-100 font-semibold text-sm">
            <Link2 class="w-4 h-4 text-violet-400" />
            <span>{t('fk.title')}</span>
          </div>
          <button class="p-1 rounded hover:bg-[#3f3f46] text-zinc-500" onclick={closeFkModal}>
            <X class="w-4 h-4" />
          </button>
        </div>

        <div class="p-4 space-y-3 text-[11px]">
          <div class="flex items-center gap-2 font-mono text-[11px] bg-[#121214] border border-[#27272a] rounded px-3 py-2">
            <span class="text-violet-300">{linkSource.table}.{linkSource.column}</span>
            <span class="text-zinc-500">→</span>
            <span class="text-amber-300">{fkTarget.table}.{fkTarget.column}</span>
          </div>

          <label class="space-y-1 block">
            <span class="text-[10px] text-zinc-400 uppercase font-semibold">{t('fk.name')}</span>
            <input
              type="text"
              bind:value={fkName}
              class="w-full bg-[#121214] border border-[#3f3f46] rounded px-2.5 py-1.5 text-zinc-200 focus:border-violet-500 outline-none font-mono text-[11px]"
            />
          </label>

          <div class="grid grid-cols-2 gap-2">
            <label class="space-y-1 block">
              <span class="text-[10px] text-zinc-400 uppercase font-semibold">{t('fk.on_delete')}</span>
              <select
                bind:value={fkOnDelete}
                class="w-full bg-[#121214] border border-[#3f3f46] rounded px-2.5 py-1.5 text-zinc-200 focus:border-violet-500 outline-none"
              >
                {#each FK_ACTIONS as a}
                  <option value={a}>{a}</option>
                {/each}
              </select>
            </label>
            <label class="space-y-1 block">
              <span class="text-[10px] text-zinc-400 uppercase font-semibold">{t('fk.on_update')}</span>
              <select
                bind:value={fkOnUpdate}
                class="w-full bg-[#121214] border border-[#3f3f46] rounded px-2.5 py-1.5 text-zinc-200 focus:border-violet-500 outline-none"
              >
                {#each FK_ACTIONS as a}
                  <option value={a}>{a}</option>
                {/each}
              </select>
            </label>
          </div>

          <div class="space-y-1">
            <div class="text-[10px] text-zinc-400 uppercase font-semibold">{t('fk.preview')}</div>
            <pre class="p-2.5 font-mono text-[10px] text-zinc-300 bg-[#121214] border border-[#27272a] rounded overflow-x-auto select-text">{fkPreviewSql}</pre>
          </div>

          {#if workspace.activeConnection?.driver === 'sqlite'}
            <div class="flex items-start gap-2 text-[10px] text-amber-300/90 bg-amber-500/10 border border-amber-500/30 rounded px-2.5 py-2">
              <AlertTriangle class="w-3.5 h-3.5 shrink-0 mt-0.5" />
              <span>{t('fk.sqlite_note')}</span>
            </div>
          {/if}

          {#if fkError}
            <div class="p-2.5 bg-rose-500/10 border border-rose-500/50 rounded text-rose-400 font-mono text-[10px] break-all">
              {fkError}
            </div>
          {/if}
        </div>

        <div class="px-4 py-3 border-t border-[#27272a] bg-[#141416] flex justify-end gap-2">
          <button class="px-3 py-1.5 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200" onclick={closeFkModal}>
            {t('common.cancel')}
          </button>
          <button
            class="px-4 py-1.5 rounded bg-violet-600 hover:bg-violet-500 text-white font-semibold disabled:opacity-50"
            onclick={createFk}
            disabled={fkBusy}
          >
            {fkBusy ? t('fk.creating') : t('fk.create')}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
