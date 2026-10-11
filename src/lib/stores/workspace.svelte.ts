import type {
  ConnectionProfile,
  GlobalSearchResult,
  QueryResultData,
  Snippet,
  SqlLogEntry,
  StagedCommitPayload,
  StagedDiffSummary,
  TableColumnMetadata,
  TableInfo,
  WorkspaceTab,
} from '$lib/types';
import { api } from '$lib/api';
import { withProdGuard } from '$lib/api/approval';
import { t } from '$lib/i18n/i18n.svelte';
import { toasts } from './toast.svelte';
import { confirmDialog } from './confirm.svelte';

/** Order-insensitive JSON for comparing staged payloads across previews. */
function canonicalize(value: any): any {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value !== null && typeof value === 'object') {
    return Object.keys(value)
      .sort()
      .reduce((acc: Record<string, any>, k) => {
        acc[k] = canonicalize(value[k]);
        return acc;
      }, {});
  }
  return value;
}
function canonicalJson(value: any): string {
  return JSON.stringify(canonicalize(value));
}

class WorkspaceStore {
  connections = $state<ConnectionProfile[]>([]);
  activeConnection = $state<ConnectionProfile | null>(null);
  isConnected = $state(false);
  isConnecting = $state(false);

  tables = $state<TableInfo[]>([]);
  activeTable = $state<string | null>(null);

  tabs = $state<WorkspaceTab[]>([]);
  activeTabId = $state<string | null>(null);

  sqlLogs = $state<SqlLogEntry[]>([]);
  isLogConsoleOpen = $state(false);

  serverVersion = $state('');

  // Schema cache for SQL autocomplete: tableName -> columns
  schemaCache = $state<Record<string, TableColumnMetadata[]>>({});

  // Modals
  isConnectionModalOpen = $state(false);
  isCodeReviewModalOpen = $state(false);
  isGlobalSearchModalOpen = $state(false);
  isMaintenanceModalOpen = $state(false);
  isQuickSwitcherOpen = $state(false);
  isImportCsvModalOpen = $state(false);
  isCreateTableModalOpen = $state(false);
  isAiSettingsOpen = $state(false);

  // Saved SQL snippets (persisted via vault)
  snippets = $state<Snippet[]>([]);

  // ER diagram node positions: tableName -> {x, y}
  erdPositions = $state<Record<string, { x: number; y: number }>>({});

  // Context of the staged changes currently being reviewed in the diff modal,
  // so the commit uses the real primary keys instead of a hardcoded guess.
  reviewTable = $state<string | null>(null);
  reviewPrimaryKeys = $state<string[]>([]);

  // Staged changes per table
  // Key: tableName, Value: { inserts: [], updates: [], deletes: [] }
  stagedChanges = $state<
    Record<
      string,
      {
        inserts: Record<string, any>[];
        updates: Record<string, Record<string, any>>[]; // list of { pk: Record, fields: Record }
        deletes: Record<string, any>[];
      }
    >
  >({});

  stagedDiff = $state<StagedDiffSummary | null>(null);
  isCommitting = $state(false);
  // Exact payload the user reviewed; commit refuses anything different.
  private reviewPayload: StagedCommitPayload | null = null;
  // Bumped on every connect/disconnect: async results issued before the
  // bump are discarded instead of writing into a replaced workspace.
  private epoch = 0;

  /// Staged mutations or unsaved query drafts that switching/disconnecting
  /// would silently drop.
  hasUnsavedWork(): boolean {
    const staged = Object.values(this.stagedChanges).some(
      (s) => s.inserts.length + s.updates.length + s.deletes.length > 0
    );
    return staged || this.tabs.some((t) => t.isDirty);
  }

  /// Wipe everything scoped to the previous connection. Only called AFTER
  /// a new connection is proven live (or on explicit disconnect).
  private clearWorkspaceState() {
    this.tables = [];
    this.activeTable = null;
    this.tabs = [];
    this.activeTabId = null;
    this.stagedChanges = {};
    this.stagedDiff = null;
    this.reviewPayload = null;
    this.reviewTable = null;
    this.reviewPrimaryKeys = [];
    this.schemaCache = {};
    this.erdPositions = {};
    this.serverVersion = '';
    this.isCodeReviewModalOpen = false;
  }

  async init() {
    await this.refreshConnections();
    this.refreshSnippets();
    // Default open at least 1 connection if exists
    if (this.connections.length > 0 && !this.activeConnection) {
      await this.connect(this.connections[0].id);
    } else if (this.connections.length === 0) {
      // Create a default Demo SQLite in-memory connection
      const demo: ConnectionProfile = {
        id: 'demo-sqlite',
        name: 'Local Demo SQLite',
        driver: 'sqlite',
        environment: 'local',
        file_path: ':memory:',
      };
      await api.saveConnectionProfile(demo);
      await this.refreshConnections();
      await this.connect(demo.id);
      await this.seedDemoData();
    }
  }

  async refreshConnections() {
    this.connections = await api.getConnectionProfiles();
  }

  /// Returns true when the connection became active — callers can gate UX on it.
  /// `password` optionally bypasses the keychain (fresh from the form).
  ///
  /// Switching order matters: confirm unsaved work → attempt target →
  /// only on success clear the old workspace. Cancelled or failed
  /// connections leave the previous workspace fully intact.
  async connect(connectionId: string, password?: string): Promise<boolean> {
    const conn = this.connections.find((c) => c.id === connectionId);
    if (!conn) return false;
    const switching = !!this.activeConnection && this.activeConnection.id !== connectionId;

    if (switching && this.hasUnsavedWork()) {
      const ok = await confirmDialog.ask({
        title: t('confirm.switch_conn_title'),
        message: t('confirm.switch_conn_msg', { name: conn.name }),
        confirmLabel: t('confirm.switch_conn'),
        danger: true,
      });
      if (!ok) return false;
    }

    this.isConnecting = true;
    const epoch = ++this.epoch;
    try {
      await api.connectDatabase(connectionId, password);
      // A newer connect/disconnect started while this one was in flight —
      // its epoch wins; drop this result instead of corrupting the state.
      if (epoch !== this.epoch) return false;

      if (switching) this.clearWorkspaceState();
      this.activeConnection = conn;
      this.isConnected = true;
      await this.refreshTables();
      await this.refreshLogs();
      await this.refreshSchemaCache();
      api.getServerVersion(connectionId).then((v) => {
        if (this.activeConnection?.id === connectionId) this.serverVersion = v;
      }).catch(() => {
        if (this.activeConnection?.id === connectionId) this.serverVersion = '';
      });

      // If no tabs open, open default query tab
      if (this.tabs.length === 0) {
        this.openQueryTab();
      }
      return true;
    } catch (err: any) {
      toasts.error(t('common.connect_failed') + err);
      return false;
    } finally {
      this.isConnecting = false;
    }
  }

  async disconnect() {
    const conn = this.activeConnection;
    if (conn && this.hasUnsavedWork()) {
      const ok = await confirmDialog.ask({
        title: t('confirm.disconnect_title'),
        message: t('confirm.disconnect_msg', { name: conn.name }),
        confirmLabel: t('confirm.disconnect'),
        danger: true,
      });
      if (!ok) return;
    }
    this.epoch++; // invalidate every in-flight request against the old session
    if (conn) {
      await api.disconnectDatabase(conn.id);
    }
    this.activeConnection = null;
    this.isConnected = false;
    this.clearWorkspaceState();
    await this.refreshLogs();
  }

  async refreshTables() {
    const connId = this.activeConnection?.id;
    if (!connId) return;
    const tables = await api.listTables(connId);
    if (this.activeConnection?.id === connId) this.tables = tables;
  }

  // ---- Snippets ----
  async refreshSnippets() {
    this.snippets = await api.listSnippets();
  }

  async saveSnippet(name: string, sql: string) {
    const snippet: Snippet = {
      id: 'snip-' + Math.random().toString(36).substring(2, 9),
      name,
      sql,
      created_at: new Date().toISOString(),
    };
    await api.saveSnippet(snippet);
    await this.refreshSnippets();
  }

  async deleteSnippet(id: string) {
    await api.deleteSnippet(id);
    await this.refreshSnippets();
  }

  /// Fetch every table's columns once so the SQL editor can autocomplete
  /// real table/column names. Refreshed on connect and after DDL-affecting commits.
  async refreshSchemaCache() {
    const connId = this.activeConnection?.id;
    if (!connId) return;
    const cache: Record<string, TableColumnMetadata[]> = {};
    for (const t of this.tables) {
      try {
        cache[t.name] = await api.getTableSchema(connId, t.name);
      } catch {
        cache[t.name] = [];
      }
    }
    if (this.activeConnection?.id === connId) this.schemaCache = cache;
  }

  async refreshLogs() {
    this.sqlLogs = await api.getSqlLogs();
  }

  openTableTab(tableName: string) {
    const existing = this.tabs.find((t) => t.type === 'table' && t.tableName === tableName);
    if (existing) {
      this.activeTabId = existing.id;
      return;
    }
    const newTab: WorkspaceTab = {
      id: 'tab-' + Math.random().toString(36).substring(2, 9),
      title: tableName,
      type: 'table',
      connectionId: this.activeConnection?.id,
      tableName,
    };
    this.tabs.push(newTab);
    this.activeTabId = newTab.id;
    this.activeTable = tableName;
  }

  openStructureTab(tableName: string) {
    const tabId = 'struct-' + tableName;
    const existing = this.tabs.find((t) => t.id === tabId);
    if (existing) {
      this.activeTabId = existing.id;
      return;
    }
    const newTab: WorkspaceTab = {
      id: tabId,
      title: tableName + ' (Structure)',
      type: 'structure',
      connectionId: this.activeConnection?.id,
      tableName,
    };
    this.tabs.push(newTab);
    this.activeTabId = newTab.id;
  }

  openQueryTab(query: string = 'SELECT * FROM users LIMIT 50;') {
    const count = this.tabs.filter((t) => t.type === 'query').length + 1;
    const newTab: WorkspaceTab = {
      id: 'tab-query-' + Math.random().toString(36).substring(2, 9),
      title: t('tab.query', { count }),
      type: 'query',
      connectionId: this.activeConnection?.id,
      queryText: query,
    };
    this.tabs.push(newTab);
    this.activeTabId = newTab.id;
  }

  /// Sync a live editor draft into tab state so dirty-work checks (switch
  /// connection, disconnect, close tab) see the real contents.
  updateTabDraft(tabId: string, text: string) {
    const tab = this.tabs.find((t) => t.id === tabId);
    if (!tab || tab.queryText === text) return;
    tab.queryText = text;
    tab.isDirty = true;
  }

  openAuditTab() {
    const existing = this.tabs.find((t) => t.type === 'audit');
    if (existing) {
      this.activeTabId = existing.id;
      return;
    }
    this.tabs.push({ id: 'tab-audit', title: t('tab.audit'), type: 'audit' });
    this.activeTabId = 'tab-audit';
  }

  openErdTab() {
    const existing = this.tabs.find((t) => t.type === 'erd');
    if (existing) {
      this.activeTabId = existing.id;
      return;
    }
    this.tabs.push({ id: 'tab-erd', title: t('tab.erd'), type: 'erd' });
    this.activeTabId = 'tab-erd';
  }

  openDashboardTab() {
    const existing = this.tabs.find((t) => t.type === 'dashboard');
    if (existing) {
      this.activeTabId = existing.id;
      return;
    }
    this.tabs.push({ id: 'tab-dashboard', title: t('tab.dashboard'), type: 'dashboard' });
    this.activeTabId = 'tab-dashboard';
  }

  closeTab(tabId: string) {
    const idx = this.tabs.findIndex((t) => t.id === tabId);
    if (idx === -1) return;

    this.tabs.splice(idx, 1);
    if (this.activeTabId === tabId) {
      if (this.tabs.length > 0) {
        this.activeTabId = this.tabs[Math.max(0, idx - 1)].id;
      } else {
        this.activeTabId = null;
      }
    }
  }

  closeOtherTabs(tabId: string) {
    const keep = this.tabs.find((t) => t.id === tabId);
    if (!keep) return;
    this.tabs = [keep];
    this.activeTabId = tabId;
  }

  closeTabsToRight(tabId: string) {
    const idx = this.tabs.findIndex((t) => t.id === tabId);
    if (idx === -1) return;
    this.tabs.splice(idx + 1);
    if (!this.tabs.some((t) => t.id === this.activeTabId)) {
      this.activeTabId = tabId;
    }
  }

  closeAllTabs() {
    this.tabs = [];
    this.activeTabId = null;
  }

  /// Confirmation gate before closing tabs: warns (once) when the tabs being
  /// closed carry staged changes or unsaved drafts, then runs `apply` if
  /// the user confirms.
  async requestCloseTabs(tabIds: string[], apply: () => void) {
    const tables = new Set(
      tabIds
        .map((id) => this.tabs.find((tb) => tb.id === id)?.tableName)
        .filter((n): n is string => !!n)
    );
    const stagedCount = [...tables].reduce(
      (n, name) => n + this.getTotalStagedCount(name),
      0
    );
    const dirtyDrafts = tabIds.filter(
      (id) => this.tabs.find((tb) => tb.id === id)?.isDirty
    ).length;
    const riskCount = stagedCount + dirtyDrafts;
    if (riskCount > 0) {
      const ok = await confirmDialog.ask({
        title: t('confirm.close_staged_title'),
        message: t('confirm.close_staged_msg', { count: riskCount }),
        confirmLabel: t('confirm.close_tab'),
        danger: true,
      });
      if (!ok) return;
    }
    apply();
  }

  // Staged changes management (pure read-only to avoid Svelte 5 state_unsafe_mutation)
  getStagedForTable(table: string) {
    return this.stagedChanges[table] ?? { inserts: [], updates: [], deletes: [] };
  }

  private ensureStaged(table: string) {
    if (!this.stagedChanges[table]) {
      this.stagedChanges[table] = { inserts: [], updates: [], deletes: [] };
    }
    return this.stagedChanges[table];
  }

  stageUpdate(table: string, pkValues: Record<string, any>, updatedFields: Record<string, any>) {
    const staged = this.ensureStaged(table);
    // Find if update already exists for this PK
    const pkKey = JSON.stringify(pkValues);
    const existingIdx = staged.updates.findIndex((u) => JSON.stringify(u.pk) === pkKey);

    if (existingIdx !== -1) {
      staged.updates[existingIdx].fields = {
        ...staged.updates[existingIdx].fields,
        ...updatedFields,
      };
    } else {
      staged.updates.push({ pk: pkValues, fields: updatedFields });
    }
  }

  stageInsert(table: string, newRow: Record<string, any>) {
    const staged = this.ensureStaged(table);
    staged.inserts.push(newRow);
  }

  stageDelete(table: string, pkValues: Record<string, any>) {
    const staged = this.ensureStaged(table);
    staged.deletes.push(pkValues);
  }

  discardChanges(table: string) {
    if (this.stagedChanges[table]) {
      this.stagedChanges[table] = { inserts: [], updates: [], deletes: [] };
    }
  }

  getTotalStagedCount(table: string): number {
    const staged = this.stagedChanges[table];
    if (!staged) return 0;
    return staged.inserts.length + staged.updates.length + staged.deletes.length;
  }

  private buildStagedPayload(table: string, primaryKeys: string[]): StagedCommitPayload | null {
    if (!this.activeConnection) return null;
    const staged = this.getStagedForTable(table);
    return {
      connection_id: this.activeConnection.id,
      table_name: table,
      primary_keys: primaryKeys,
      inserts: staged.inserts,
      updates: staged.updates.map((u) => ({
        primary_key_values: u.pk,
        updated_fields: u.fields,
      })),
      deletes: staged.deletes,
    };
  }

  async previewStaged(table: string, primaryKeys: string[]) {
    const payload = this.buildStagedPayload(table, primaryKeys);
    if (!payload) return;
    this.reviewPayload = payload;
    this.reviewTable = table;
    this.reviewPrimaryKeys = primaryKeys;
    this.stagedDiff = await api.previewStagedChanges(payload);
    this.isCodeReviewModalOpen = true;
  }

  async commitStaged(table: string, primaryKeys: string[]) {
    // No implicit retry/double-execution: a second click while a commit is
    // in flight is ignored, and any payload change after preview forces a
    // fresh review instead of silently committing different statements.
    if (this.isCommitting) return;
    const payload = this.buildStagedPayload(table, primaryKeys);
    if (!payload) return;

    if (!this.reviewPayload || canonicalJson(payload) !== canonicalJson(this.reviewPayload)) {
      toasts.info(t('review.stale'));
      await this.previewStaged(table, primaryKeys);
      return;
    }

    this.isCommitting = true;
    try {
      await withProdGuard((tok) => api.commitStagedChanges(payload, tok));
      this.discardChanges(table);
      this.reviewPayload = null;
      this.isCodeReviewModalOpen = false;
      await this.refreshTables();
      await this.refreshLogs();
    } catch (err: any) {
      toasts.error(t('common.commit_failed') + err);
    } finally {
      this.isCommitting = false;
    }
  }

  private async seedDemoData() {
    if (!this.activeConnection) return;
    const seedSql = `
      CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        email TEXT UNIQUE,
        role TEXT DEFAULT 'developer',
        balance REAL DEFAULT 1500.50,
        is_active INTEGER DEFAULT 1,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
      );

      CREATE TABLE IF NOT EXISTS projects (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER,
        title TEXT NOT NULL,
        status TEXT DEFAULT 'in_progress',
        budget REAL,
        FOREIGN KEY (user_id) REFERENCES users(id)
      );

      INSERT OR IGNORE INTO users (id, name, email, role, balance, is_active) VALUES
      (1, 'John Doe', 'john@company.com', 'admin', 5400.00, 1),
      (2, 'Jane Miller', 'jane@design.io', 'lead_designer', 4200.75, 1),
      (3, 'Robert Fox', 'robert@backend.net', 'developer', 3100.00, 1),
      (4, 'Emily Watson', 'emily@qa.org', 'qa_engineer', 2800.50, 0),
      (5, 'David Kim', 'david@security.co', 'devops', 4850.00, 1);

      INSERT OR IGNORE INTO projects (id, user_id, title, status, budget) VALUES
      (1, 1, 'TablePro Desktop App', 'in_progress', 15000),
      (2, 3, 'High Performance SMT Engine', 'completed', 32000),
      (3, 5, 'Cloud Infrastructure Setup', 'in_progress', 8500);
    `;
    await withProdGuard((tok) => api.executeQuery(this.activeConnection!.id, seedSql, tok));
    await this.refreshTables();
  }
}

export const workspace = new WorkspaceStore();
