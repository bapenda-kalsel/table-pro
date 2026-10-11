<script lang="ts">
  import { workspace } from '$lib/stores/workspace.svelte';
  import { api } from '$lib/api';
  import type { ConnectionProfile, DatabaseDriverType, EnvironmentTag } from '$lib/types';
  import { Database, Plus, Trash2, CheckCircle2, AlertCircle, X, Lock, FolderOpen, Plug, Network } from 'lucide-svelte';
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
  let formSslMode = $state('verify_full');
  let formSslCa = $state('');

  // SSH tunnel state (TCP drivers only)
  let sshEnabled = $state(false);
  let sshHost = $state('');
  let sshPort = $state(22);
  let sshUsername = $state('');
  let sshAuthMethod = $state<'password' | 'key_file'>('password');
  let sshKeyPath = $state('');
  let sshPassword = $state('');
  let sshPassphrase = $state('');

  let testResult = $state<{ success: boolean; message: string } | null>(null);
  let isTesting = $state(false);

  function buildProfile(id: string): ConnectionProfile {
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
    // TLS fields only apply to TCP drivers; the backend ignores them for
    // SQLite but would reject a missing ssl_mode for TCP.
    if (formDriver !== 'sqlite') {
      profile.ssl_mode = formSslMode;
      if (formSslCa.trim()) profile.ssl_ca_cert_path = formSslCa.trim();
      if (sshEnabled) {
        profile.ssh = {
          enabled: true,
          host: sshHost,
          port: sshPort || 22,
          username: sshUsername,
          authMethod: sshAuthMethod,
          keyPath: sshAuthMethod === 'key_file' ? sshKeyPath : undefined,
        };
      } else {
        profile.ssh = { enabled: false, host: '', username: '', authMethod: 'password' };
      }
    }
    return profile;
  }

  /** Inline SSH secrets for save/test. Disabled SSH clears stored secrets. */
  function sshSecrets() {
    if (formDriver === 'sqlite') return undefined;
    return {
      sshPassword: sshEnabled ? sshPassword || undefined : '',
      sshPassphrase: sshEnabled && sshAuthMethod === 'key_file' ? sshPassphrase || undefined : '',
    };
  }

  async function pickCaFile() {
    if (!isTauri) return;
    const selected = await open({
      title: t('conn.tls_ca'),
      multiple: false,
      filters: [{ name: 'PEM / CRT', extensions: ['pem', 'crt', 'cer'] }],
    });
    if (typeof selected === 'string') {
      formSslCa = selected;
    }
  }

  async function pickSshKeyFile() {
    if (!isTauri) return;
    const selected = await open({ title: t('conn.ssh_key_pick'), multiple: false });
    if (typeof selected === 'string') {
      sshKeyPath = selected;
    }
  }

  function resetForm() {
    selectedProfileId = null;
    formName = 'New Database Connection';
    formDriver = 'sqlite';
    formEnv = 'local';
    formFilePath = ':memory:';
    formPassword = '';
    formSslMode = 'verify_full';
    formSslCa = '';
    sshEnabled = false;
    sshHost = '';
    sshPort = 22;
    sshUsername = '';
    sshAuthMethod = 'password';
    sshKeyPath = '';
    sshPassword = '';
    sshPassphrase = '';
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
    // Legacy profiles have no ssl_mode; prefill the strict default so the
    // next save migrates them explicitly (backend rejects it otherwise).
    formSslMode = p.ssl_mode || 'verify_full';
    formSslCa = p.ssl_ca_cert_path || '';
    sshEnabled = p.ssh?.enabled ?? false;
    sshHost = p.ssh?.host ?? '';
    sshPort = p.ssh?.port ?? 22;
    sshUsername = p.ssh?.username ?? '';
    sshAuthMethod = (p.ssh?.authMethod as 'password' | 'key_file') ?? 'password';
    sshKeyPath = p.ssh?.keyPath ?? '';
    // SSH secrets live in the keychain — fields stay blank; blank = keep.
    sshPassword = '';
    sshPassphrase = '';
    testResult = null;
  }

  async function handleTest() {
    isTesting = true;
    testResult = null;
    const profile = buildProfile(selectedProfileId || 'temp-id');

    try {
      const pingMs = await api.testConnection(profile, formPassword || undefined, sshSecrets());
      testResult = { success: true, message: t('conn.test_ok', { ms: pingMs.toFixed(2) }) };
    } catch (err: any) {
      testResult = { success: false, message: String(err) };
    } finally {
      isTesting = false;
    }
  }

  async function handleSave(): Promise<string | null> {
    const id = selectedProfileId || 'conn-' + Math.random().toString(36).substring(2, 9);
    const profile = buildProfile(id);

    try {
      await api.saveConnectionProfile(profile, formPassword || undefined, sshSecrets());
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
    if (ok) {
      formPassword = '';
      workspace.isConnectionModalOpen = false;
    }
  }
</script>

{#if workspace.isConnectionModalOpen}
  <div class="fixed inset-0 bg-black/70 backdrop-blur-xs flex items-center justify-center z-50 p-4">
    <div class="w-[740px] h-[520px] bg-panel border border-edge rounded-xl shadow-2xl flex flex-col text-xs overflow-hidden select-none">
      <!-- Header -->
      <div class="h-12 border-b border-edge px-4 flex items-center justify-between bg-subtle">
        <div class="flex items-center gap-2 font-bold text-fg text-sm">
          <Database class="w-4 h-4 text-sky-400" />
          <span>{t('conn.title')}</span>
        </div>
        <button
          class="p-1 rounded hover:bg-hover text-muted hover:text-fg"
          onclick={() => (workspace.isConnectionModalOpen = false)}
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <!-- Main Body: 2 Columns -->
      <div class="flex-1 flex overflow-hidden">
        <!-- Left Column: Profiles List -->
        <div class="w-56 border-r border-edge bg-subtle flex flex-col p-2 space-y-1">
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
                    ? 'bg-elevated text-fg font-semibold border-l-2 border-sky-500'
                    : 'hover:bg-subtle text-muted hover:text-fg'
                }`}
                onclick={() => selectProfile(conn)}
                onkeydown={(e) => e.key === 'Enter' && selectProfile(conn)}
                tabindex="0"
                role="button"
              >
                <div class="truncate">
                  <div class="truncate">{conn.name}</div>
                  <div class="text-[9px] uppercase tracking-wider text-faint font-mono">
                    {conn.driver} • {conn.environment}
                  </div>
                </div>

                <div class="flex items-center gap-0.5 shrink-0">
                  <button
                    class="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-hover text-muted hover:text-emerald-400 transition-opacity"
                    onclick={(e) => {
                      e.stopPropagation();
                      handleConnectExisting(conn.id);
                    }}
                    title={t('conn.connect_hint')}
                  >
                    <Plug class="w-3 h-3" />
                  </button>
                  <button
                    class="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-hover text-muted hover:text-rose-400 transition-opacity"
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
        <div class="flex-1 overflow-y-auto p-4 space-y-4 bg-panel">
          <div class="space-y-1">
            <label for="conn-name" class="text-[10px] text-muted uppercase font-semibold">{t('conn.name')}</label>
            <input
              type="text"
              id="conn-name" bind:value={formName}
              placeholder={t('conn.name_ph')}
              class="w-full bg-app border border-edge rounded px-3 py-1.5 text-fg focus:outline-none focus:border-sky-500"
            />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-1">
              <label for="conn-driver" class="text-[10px] text-muted uppercase font-semibold">{t('conn.driver')}</label>
              <select
                id="conn-driver" bind:value={formDriver}
                onchange={() => {
                  if (formDriver === 'mysql') formPort = 3306;
                  else if (formDriver === 'postgres') formPort = 5432;
                  if (formDriver !== 'sqlite' && !formDatabase) formDatabase = 'postgres';
                }}
                class="w-full bg-app border border-edge rounded px-3 py-1.5 text-fg focus:outline-none focus:border-sky-500"
              >
                <option value="sqlite">{t('conn.driver_sqlite')}</option>
                <option value="mysql">MySQL / MariaDB</option>
                <option value="postgres">PostgreSQL</option>
              </select>
            </div>

            <div class="space-y-1">
              <label for="conn-env" class="text-[10px] text-muted uppercase font-semibold">{t('conn.env')}</label>
              <select
                id="conn-env" bind:value={formEnv}
                class="w-full bg-app border border-edge rounded px-3 py-1.5 text-fg focus:outline-none focus:border-sky-500"
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
                <label for="conn-path" class="text-[10px] text-muted uppercase font-semibold">{t('conn.path')}</label>
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
                  id="conn-path" bind:value={formFilePath}
                  placeholder={t('conn.path_ph')}
                  class="flex-1 bg-app border border-edge rounded px-3 py-1.5 text-fg focus:outline-none focus:border-sky-500 font-mono text-[11px]"
                />
                <button
                  type="button"
                  class="flex items-center gap-1 px-3 py-1.5 rounded bg-elevated hover:bg-hover text-fg transition-colors"
                  onclick={pickDbFile}
                  title={t('conn.browse_hint')}
                >
                  <FolderOpen class="w-3.5 h-3.5" /><span>{t('common.browse')}</span>
                </button>
              </div>
              <p class="text-[10px] text-faint">
                {t('conn.auto_create')}
              </p>
            </div>
          {:else}
            <!-- Host, Port, DB, User -->
            <div class="grid grid-cols-3 gap-2">
              <div class="col-span-2 space-y-1">
                <label for="conn-host" class="text-[10px] text-muted uppercase font-semibold">{t('conn.host')}</label>
                <input
                  type="text"
                  id="conn-host" bind:value={formHost}
                  class="w-full bg-app border border-edge rounded px-2.5 py-1.5 text-fg"
                />
              </div>
              <div class="space-y-1">
                <label for="conn-port" class="text-[10px] text-muted uppercase font-semibold">{t('conn.port')}</label>
                <input
                  type="number"
                  id="conn-port" bind:value={formPort}
                  class="w-full bg-app border border-edge rounded px-2.5 py-1.5 text-fg"
                />
              </div>
            </div>

            <div class="grid grid-cols-2 gap-2">
              <div class="space-y-1">
                <label for="conn-db" class="text-[10px] text-muted uppercase font-semibold">{t('conn.db')}</label>
                <input
                  type="text"
                  id="conn-db" bind:value={formDatabase}
                  class="w-full bg-app border border-edge rounded px-2.5 py-1.5 text-fg"
                />
              </div>
              <div class="space-y-1">
                <label for="conn-user" class="text-[10px] text-muted uppercase font-semibold">{t('conn.user')}</label>
                <input
                  type="text"
                  id="conn-user" bind:value={formUsername}
                  class="w-full bg-app border border-edge rounded px-2.5 py-1.5 text-fg"
                />
              </div>
            </div>

            <div class="space-y-1">
              <div class="flex items-center gap-1 text-[10px] text-muted uppercase font-semibold">
                <Lock class="w-3 h-3 text-sky-400" />
                <span>{t('conn.password')}</span>
              </div>
              <input
                type="password"
                bind:value={formPassword}
                placeholder={t('conn.password_ph')}
                class="w-full bg-app border border-edge rounded px-3 py-1.5 text-fg focus:outline-none"
              />
            </div>

            <div class="space-y-1">
              <label for="conn-tls" class="text-[10px] text-muted uppercase font-semibold">{t('conn.tls_mode')}</label>
              <select
                id="conn-tls" bind:value={formSslMode}
                class="w-full bg-app border border-edge rounded px-3 py-1.5 text-fg focus:outline-none focus:border-sky-500"
              >
                <option value="verify_full">{t('conn.tls_verify_full')}</option>
                <option value="verify_ca">{t('conn.tls_verify_ca')}</option>
                <option value="require">{t('conn.tls_require')}</option>
                <option value="prefer">{t('conn.tls_prefer')}</option>
                <option value="disable">{t('conn.tls_disable')}</option>
              </select>
            </div>

            {#if formSslMode === 'verify_ca' || formSslMode === 'verify_full'}
              <div class="space-y-1">
                <label for="conn-ca" class="text-[10px] text-muted uppercase font-semibold">{t('conn.tls_ca')}</label>
                <div class="flex gap-2">
                  <input
                    type="text"
                    id="conn-ca" bind:value={formSslCa}
                    placeholder={t('conn.tls_ca_ph')}
                    class="flex-1 bg-app border border-edge rounded px-3 py-1.5 text-fg focus:outline-none font-mono text-[11px]"
                  />
                  <button
                    type="button"
                    class="flex items-center gap-1 px-3 py-1.5 rounded bg-elevated hover:bg-hover text-fg transition-colors"
                    onclick={pickCaFile}
                    title={t('conn.tls_browse_hint')}
                  >
                    <FolderOpen class="w-3.5 h-3.5" /><span>{t('common.browse')}</span>
                  </button>
                </div>
                <p class="text-[10px] text-faint">{t('conn.tls_ca_hint')}</p>
              </div>
            {/if}

            <!-- SSH Tunnel -->
            <div class="space-y-3 rounded-lg border border-edge bg-subtle p-3">
              <label class="flex items-center gap-2 text-[10px] text-muted uppercase font-semibold cursor-pointer">
                <input type="checkbox" bind:checked={sshEnabled} class="rounded" />
                <Network class="w-3 h-3 text-violet-400" />
                {t('conn.ssh_toggle')}
              </label>

              {#if sshEnabled}
                <div class="grid grid-cols-3 gap-2">
                  <div class="col-span-2 space-y-1">
                    <label for="ssh-host" class="text-[10px] text-muted uppercase font-semibold">{t('conn.ssh_host')}</label>
                    <input
                      id="ssh-host"
                      type="text"
                      bind:value={sshHost}
                      placeholder="bastion.example.com"
                      class="w-full bg-app border border-edge rounded px-2.5 py-1.5 text-fg"
                    />
                  </div>
                  <div class="space-y-1">
                    <label for="ssh-port" class="text-[10px] text-muted uppercase font-semibold">{t('conn.ssh_port')}</label>
                    <input
                      id="ssh-port"
                      type="number"
                      bind:value={sshPort}
                      class="w-full bg-app border border-edge rounded px-2.5 py-1.5 text-fg"
                    />
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-2">
                  <div class="space-y-1">
                    <label for="ssh-user" class="text-[10px] text-muted uppercase font-semibold">{t('conn.ssh_user')}</label>
                    <input
                      id="ssh-user"
                      type="text"
                      bind:value={sshUsername}
                      class="w-full bg-app border border-edge rounded px-2.5 py-1.5 text-fg"
                    />
                  </div>
                  <div class="space-y-1">
                    <label for="ssh-auth" class="text-[10px] text-muted uppercase font-semibold">{t('conn.ssh_auth')}</label>
                    <select
                      id="ssh-auth"
                      bind:value={sshAuthMethod}
                      class="w-full bg-app border border-edge rounded px-2.5 py-1.5 text-fg focus:outline-none focus:border-sky-500"
                    >
                      <option value="password">{t('conn.ssh_auth_pw')}</option>
                      <option value="key_file">{t('conn.ssh_auth_key')}</option>
                    </select>
                  </div>
                </div>

                {#if sshAuthMethod === 'password'}
                  <div class="space-y-1">
                    <label for="ssh-pass" class="text-[10px] text-muted uppercase font-semibold">{t('conn.ssh_password')}</label>
                    <input
                      id="ssh-pass"
                      type="password"
                      bind:value={sshPassword}
                      placeholder={t('conn.ssh_password_ph')}
                      class="w-full bg-app border border-edge rounded px-3 py-1.5 text-fg focus:outline-none"
                    />
                  </div>
                {:else}
                  <div class="space-y-1">
                    <label for="ssh-key" class="text-[10px] text-muted uppercase font-semibold">{t('conn.ssh_key')}</label>
                    <div class="flex gap-2">
                      <input
                        id="ssh-key"
                        type="text"
                        bind:value={sshKeyPath}
                        placeholder="~/.ssh/id_ed25519"
                        class="flex-1 bg-app border border-edge rounded px-3 py-1.5 text-fg focus:outline-none font-mono text-[11px]"
                      />
                      <button
                        type="button"
                        class="flex items-center gap-1 px-3 py-1.5 rounded bg-elevated hover:bg-hover text-fg transition-colors"
                        onclick={pickSshKeyFile}
                      >
                        <FolderOpen class="w-3.5 h-3.5" /><span>{t('common.browse')}</span>
                      </button>
                    </div>
                  </div>
                  <div class="space-y-1">
                    <label for="ssh-pp" class="text-[10px] text-muted uppercase font-semibold">{t('conn.ssh_passphrase')}</label>
                    <input
                      id="ssh-pp"
                      type="password"
                      bind:value={sshPassphrase}
                      placeholder={t('conn.ssh_passphrase_ph')}
                      class="w-full bg-app border border-edge rounded px-3 py-1.5 text-fg focus:outline-none"
                    />
                  </div>
                {/if}

                <p class="text-[10px] text-faint">
                  {t('conn.ssh_note')}
                </p>
              {/if}
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
      <div class="h-12 border-t border-edge px-4 flex items-center justify-between bg-subtle">
        <button
          class="px-3 py-1.5 rounded bg-elevated hover:bg-hover text-fg2 font-medium transition-colors disabled:opacity-50"
          onclick={handleTest}
          disabled={isTesting}
        >
          {isTesting ? t('conn.testing') : t('conn.test')}
        </button>

        <div class="flex items-center gap-2">
          <button
            class="px-3 py-1.5 rounded bg-elevated hover:bg-hover text-fg2 transition-colors"
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
