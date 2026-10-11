<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import type { ConnectionProfile, DatabaseDriverType, EnvironmentTag } from '$lib/types';
  import { Database, Plus, Trash2, CheckCircle2, AlertCircle, X, Lock, FolderOpen, Plug } from 'lucide-svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { t } from '$lib/i18n/i18n.svelte';
  import { confirmDialog } from '$lib/stores/confirm.svelte';

  const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

  async function pickDbFile() {
    if (!isTauri) {
      formFilePath = '/mock/path/database.db';
      return;
    }
    const selected = await open({
      title: t('conn.pick_title'),
      multiple: false,
      filters: [{ name: t('conn.pick_filter'), extensions: ['db', 'sqlite', 'sqlite3', 'db3'] }],
    });
    if (typeof selected === 'string') {
      formFilePath = selected;
    }
  }

  let selectedProfileId = $state<string | null>(null);

  // Form state
  let formName = $state('My SQLite DB');
  let formDriver = $state<DatabaseDriverType>('sqlite');
  let formEnv = $state<EnvironmentTag>('local');
  let formFilePath = $state(':memory:');
  let formHost = $state('localhost');
  let formPort = $state(3306);
  let formDatabase = $state('main');
  let formUsername = $state('root');
  let formPassword = $state('');

  let testResult = $state<{ success: boolean; message: string } | null>(null);
  let isTesting = $state(false);

  function resetForm() {
    selectedProfileId = null;
    formName = 'New Database Connection';
    formDriver = 'sqlite';
    formEnv = 'local';
    formFilePath = ':memory:';
    formPassword = '';
    testResult = null;
  }

  function selectProfile(p: ConnectionProfile) {
    selectedProfileId = p.id;
    formName = p.name;
    formDriver = p.driver;
    formEnv = p.environment;
    formFilePath = p.file_path || ':memory:';
    formHost = p.host || 'localhost';
    formPort = p.port || (p.driver === 'mysql' ? 3306 : 5432);
    formDatabase = p.database || '';
    formUsername = p.username || '';
    formPassword = '';
    testResult = null;
  }

  async function handleTest() {
    isTesting = true;
    testResult = null;
    const profile: ConnectionProfile = {
      id: selectedProfileId || 'temp-id',
      name: formName,
      driver: formDriver,
      environment: formEnv,
      file_path: formFilePath,
      host: formHost,
      port: formPort,
      database: formDatabase,
      username: formUsername,
    };

    try {
      const pingMs = await api.testConnection(profile, formPassword || undefined);
      testResult = { success: true, message: t('conn.test_ok', { ms: pingMs.toFixed(2) }) };
    } catch (err: any) {
      testResult = { success: false, message: String(err) };
    } finally {
      isTesting = false;
    }
  }

  async function handleSave(): Promise<string | null> {
    const id = selectedProfileId || 'conn-' + Math.random().toString(36).substring(2, 9);
    const profile: ConnectionProfile = {
      id,
      name: formName,
      driver: formDriver,
      environment: formEnv,
      file_path: formFilePath,
      host: formHost,
      port: formPort,
      database: formDatabase,
      username: formUsername,
    };

    try {
      await api.saveConnectionProfile(profile, formPassword || undefined);
      await workspace.refreshConnections();
      selectedProfileId = id;
      testResult = { success: true, message: t('conn.saved') };
      return id;
    } catch (err: any) {
      testResult = { success: false, message: String(err) };
      return null;
    }
  }

  async function handleDelete(id: string) {
    const ok = await confirmDialog.ask({
      title: t('conn.delete_confirm'),
      confirmLabel: t('confirm.delete'),
      danger: true,
    });
    if (!ok) return;
    await api.deleteConnectionProfile(id);
    await workspace.refreshConnections();
    resetForm();
  }

  /// Connect directly to an existing saved profile (TablePlus-style row action).
  /// When the row is selected in the form and a password was typed, prefer it
  /// over the keychain (it may be newer than the stored credential).
  async function handleConnectExisting(id: string) {
    const ok = await workspace.connect(
      id,
      selectedProfileId === id ? formPassword || undefined : undefined,
    );
    if (ok) workspace.isConnectionModalOpen = false;
  }

  async function handleConnectAndClose() {
    const savedId = await handleSave();
    if (!savedId) return;
    const ok = await workspace.connect(savedId, formPassword || undefined);
    // Only close when the connection actually became active — otherwise the
    // user would lose the modal AND the error context.
    if (ok) workspace.isConnectionModalOpen = false;
  }
</script>

{#if workspace.isConnectionModalOpen}
  <div class="fixed inset-0 bg-black/70 backdrop-blur-xs flex items-center justify-center z-50 p-4">
    <div class="w-[740px] h-[520px] bg-[#18181b] border border-[#27272a] rounded-xl shadow-2xl flex flex-col text-xs overflow-hidden select-none">
      <!-- Header -->
      <div class="h-12 border-b border-[#27272a] px-4 flex items-center justify-between bg-[#141416]">
        <div class="flex items-center gap-2 font-bold text-zinc-100 text-sm">
          <Database class="w-4 h-4 text-sky-400" />
          <span>{t('conn.title')}</span>
        </div>
        <button
          class="p-1 rounded hover:bg-[#27272a] text-zinc-400 hover:text-zinc-200"
          onclick={() => (workspace.isConnectionModalOpen = false)}
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Main Body: 2 Columns -->
      <div class="flex-1 flex overflow-hidden">
        <!-- Left Column: Profiles List -->
        <div class="w-56 border-r border-[#27272a] bg-[#141416] flex flex-col p-2 space-y-1">
          <button
            class="flex items-center justify-center gap-1.5 w-full py-1.5 px-3 rounded bg-sky-600/20 hover:bg-sky-600/30 text-sky-400 border border-sky-500/40 font-medium transition-colors mb-2"
            onclick={resetForm}
          >
            <Plus class="w-3.5 h-3.5" />
            <span>{t('conn.new')}</span>
          </button>

          <div class="flex-1 overflow-y-auto space-y-0.5">
            {#each workspace.connections as conn}
              <div
                class={`flex items-center justify-between px-2.5 py-2 rounded cursor-pointer transition-colors group ${
                  selectedProfileId === conn.id
                    ? 'bg-[#27272a] text-zinc-100 font-semibold border-l-2 border-sky-500'
                    : 'hover:bg-[#1a1a1d] text-zinc-400 hover:text-zinc-200'
                }`}
                onclick={() => selectProfile(conn)}
                onkeydown={(e) => e.key === 'Enter' && selectProfile(conn)}
                tabindex="0"
                role="button"
              >
                <div class="truncate">
                  <div class="truncate">{conn.name}</div>
                  <div class="text-[9px] uppercase tracking-wider text-zinc-500 font-mono">
                    {conn.driver} • {conn.environment}
                  </div>
                </div>

                <div class="flex items-center gap-0.5 shrink-0">
                  <button
                    class="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-[#3f3f46] text-zinc-400 hover:text-emerald-400 transition-opacity"
                    onclick={(e) => {
                      e.stopPropagation();
                      handleConnectExisting(conn.id);
                    }}
                    title={t('conn.connect_hint')}
                  >
                    <Plug class="w-3 h-3" />
                  </button>
                  <button
                    class="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-[#3f3f46] text-zinc-400 hover:text-rose-400 transition-opacity"
                    onclick={(e) => {
                      e.stopPropagation();
                      handleDelete(conn.id);
                    }}
                    title={t('conn.delete_hint')}
                  >
                    <Trash2 class="w-3 h-3" />
                  </button>
                </div>
              </div>
            {/each}
          </div>
        </div>

        <!-- Right Column: Profile Form -->
        <div class="flex-1 overflow-y-auto p-4 space-y-4 bg-[#18181b]">
          <div class="space-y-1">
            <label class="text-[10px] text-zinc-400 uppercase font-semibold">{t('conn.name')}</label>
            <input
              type="text"
              bind:value={formName}
              placeholder={t('conn.name_ph')}
              class="w-full bg-[#121214] border border-[#2e2e33] rounded px-3 py-1.5 text-zinc-100 focus:outline-none focus:border-sky-500"
            />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-1">
              <label class="text-[10px] text-zinc-400 uppercase font-semibold">{t('conn.driver')}</label>
              <select
                bind:value={formDriver}
                onchange={() => {
                  if (formDriver === 'mysql') formPort = 3306;
                  else if (formDriver === 'postgres') formPort = 5432;
                  if (formDriver !== 'sqlite' && !formDatabase) formDatabase = 'postgres';
                }}
                class="w-full bg-[#121214] border border-[#2e2e33] rounded px-3 py-1.5 text-zinc-100 focus:outline-none focus:border-sky-500"
              >
                <option value="sqlite">{t('conn.driver_sqlite')}</option>
                <option value="mysql">MySQL / MariaDB</option>
                <option value="postgres">PostgreSQL</option>
              </select>
            </div>

            <div class="space-y-1">
              <label class="text-[10px] text-zinc-400 uppercase font-semibold">{t('conn.env')}</label>
              <select
                bind:value={formEnv}
                class="w-full bg-[#121214] border border-[#2e2e33] rounded px-3 py-1.5 text-zinc-100 focus:outline-none focus:border-sky-500"
              >
                <option value="local">{t('conn.env_local')}</option>
                <option value="staging">{t('conn.env_staging')}</option>
                <option value="production">{t('conn.env_prod')}</option>
              </select>
            </div>
          </div>

          {#if formDriver === 'sqlite'}
            <div class="space-y-1">
              <div class="flex items-center justify-between">
                <label class="text-[10px] text-zinc-400 uppercase font-semibold">{t('conn.path')}</label>
                <button
                  type="button"
                  class="text-[10px] text-sky-400 hover:underline"
                  onclick={() => (formFilePath = ':memory:')}
                >
                  {t('conn.memory')}
                </button>
              </div>
              <div class="flex gap-2">
                <input
                  type="text"
                  bind:value={formFilePath}
                  placeholder={t('conn.path_ph')}
                  class="flex-1 bg-[#121214] border border-[#2e2e33] rounded px-3 py-1.5 text-zinc-100 focus:outline-none focus:border-sky-500 font-mono text-[11px]"
                />
                <button
                  type="button"
                  class="flex items-center gap-1 px-3 py-1.5 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-200 transition-colors"
                  onclick={pickDbFile}
                  title={t('conn.browse_hint')}
                >
                  <FolderOpen class="w-3.5 h-3.5" /><span>{t('common.browse')}</span>
                </button>
              </div>
              <p class="text-[10px] text-zinc-500">
                {t('conn.auto_create')}
              </p>
            </div>
          {:else}
            <!-- Host, Port, DB, User -->
            <div class="grid grid-cols-3 gap-2">
              <div class="col-span-2 space-y-1">
                <label class="text-[10px] text-zinc-400 uppercase font-semibold">{t('conn.host')}</label>
                <input
                  type="text"
                  bind:value={formHost}
                  class="w-full bg-[#121214] border border-[#2e2e33] rounded px-2.5 py-1.5 text-zinc-100"
                />
              </div>
              <div class="space-y-1">
                <label class="text-[10px] text-zinc-400 uppercase font-semibold">{t('conn.port')}</label>
                <input
                  type="number"
                  bind:value={formPort}
                  class="w-full bg-[#121214] border border-[#2e2e33] rounded px-2.5 py-1.5 text-zinc-100"
                />
              </div>
            </div>

            <div class="grid grid-cols-2 gap-2">
              <div class="space-y-1">
                <label class="text-[10px] text-zinc-400 uppercase font-semibold">{t('conn.db')}</label>
                <input
                  type="text"
                  bind:value={formDatabase}
                  class="w-full bg-[#121214] border border-[#2e2e33] rounded px-2.5 py-1.5 text-zinc-100"
                />
              </div>
              <div class="space-y-1">
                <label class="text-[10px] text-zinc-400 uppercase font-semibold">{t('conn.user')}</label>
                <input
                  type="text"
                  bind:value={formUsername}
                  class="w-full bg-[#121214] border border-[#2e2e33] rounded px-2.5 py-1.5 text-zinc-100"
                />
              </div>
            </div>

            <div class="space-y-1">
              <div class="flex items-center gap-1 text-[10px] text-zinc-400 uppercase font-semibold">
                <Lock class="w-3 h-3 text-sky-400" />
                <span>{t('conn.password')}</span>
              </div>
              <input
                type="password"
                bind:value={formPassword}
                placeholder={t('conn.password_ph')}
                class="w-full bg-[#121214] border border-[#2e2e33] rounded px-3 py-1.5 text-zinc-100 focus:outline-none"
              />
            </div>
          {/if}

          <!-- Test Result Feedback -->
          {#if testResult}
            <div
              class={`p-2.5 rounded border text-[11px] flex items-center gap-2 ${
                testResult.success
                  ? 'bg-emerald-950/30 border-emerald-900/40 text-emerald-300'
                  : 'bg-rose-950/30 border-rose-900/40 text-rose-300'
              }`}
            >
              {#if testResult.success}
                <CheckCircle2 class="w-4 h-4 text-emerald-400 shrink-0" />
              {:else}
                <AlertCircle class="w-4 h-4 text-rose-400 shrink-0" />
              {/if}
              <span>{testResult.message}</span>
            </div>
          {/if}
        </div>
      </div>

      <!-- Footer Buttons -->
      <div class="h-12 border-t border-[#27272a] px-4 flex items-center justify-between bg-[#141416]">
        <button
          class="px-3 py-1.5 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-300 font-medium transition-colors disabled:opacity-50"
          onclick={handleTest}
          disabled={isTesting}
        >
          {isTesting ? t('conn.testing') : t('conn.test')}
        </button>

        <div class="flex items-center gap-2">
          <button
            class="px-3 py-1.5 rounded bg-[#27272a] hover:bg-[#3f3f46] text-zinc-300 transition-colors"
            onclick={handleSave}
          >
            {t('conn.save')}
          </button>
          <button
            class="px-4 py-1.5 rounded bg-sky-600 hover:bg-sky-500 text-white font-semibold shadow-md transition-colors"
            onclick={handleConnectAndClose}
          >
            {t('conn.save_connect')}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
