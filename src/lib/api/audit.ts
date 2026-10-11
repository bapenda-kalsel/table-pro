import { invoke, isTauri } from './client';
import type { AuditStats, SqlLogEntry } from '../types';

export const auditApi = {
  async getSqlLogs(): Promise<SqlLogEntry[]> {
    if (!isTauri) {
      return [
        {
          id: '1',
          timestamp: '10:15:20.120',
          query: 'SELECT * FROM users LIMIT 100 OFFSET 0;',
          duration_ms: 1.45,
          is_error: false,
        },
      ];
    }
    return invoke('get_sql_logs');
  },

  async clearSqlLogs(): Promise<void> {
    if (!isTauri) return;
    return invoke('clear_sql_logs');
  },

  async getAuditLog(params?: {
    limit?: number;
    connectionId?: string;
    text?: string;
    errorsOnly?: boolean;
    statementType?: string;
  }): Promise<SqlLogEntry[]> {
    if (!isTauri) {
      return [
        {
          id: 'a1',
          timestamp: '2026-10-08 10:15:20.120',
          query: 'SELECT * FROM users LIMIT 100 OFFSET 0;',
          duration_ms: 1.45,
          is_error: false,
          connection_name: 'Local Demo SQLite',
          environment: 'local',
          statement_type: 'SELECT',
          affected_rows: 5,
        },
        {
          id: 'a2',
          timestamp: '2026-10-08 10:16:02.441',
          query: "UPDATE users SET role = 'admin' WHERE id = 3;",
          duration_ms: 230.7,
          is_error: false,
          connection_name: 'Local Demo SQLite',
          environment: 'local',
          statement_type: 'UPDATE',
          affected_rows: 1,
        },
        {
          id: 'a3',
          timestamp: '2026-10-08 10:18:44.009',
          query: 'SELECT * FROM nonexistent;',
          duration_ms: 0.8,
          is_error: true,
          error_message: 'no such table: nonexistent',
          connection_name: 'Local Demo SQLite',
          environment: 'local',
          statement_type: 'SELECT',
        },
      ];
    }
    return invoke('get_audit_log', {
      limit: params?.limit ?? null,
      connectionId: params?.connectionId ?? null,
      text: params?.text ?? null,
      errorsOnly: params?.errorsOnly ?? null,
      statementType: params?.statementType ?? null,
    });
  },

  async getAuditStats(): Promise<AuditStats> {
    if (!isTauri) {
      return {
        total: 128,
        error_count: 3,
        avg_duration_ms: 4.2,
        max_duration_ms: 230.7,
        slowest_query: "UPDATE users SET role = 'admin' WHERE id = 3;",
        by_statement_type: { SELECT: 90, UPDATE: 20, INSERT: 10, DELETE: 5, OTHER: 3 },
      };
    }
    return invoke('get_audit_stats');
  },

  async clearAuditLog(): Promise<void> {
    if (!isTauri) return;
    return invoke('clear_audit_log');
  },

  async exportAuditLog(format: 'csv' | 'json'): Promise<string> {
    if (!isTauri) return `/mock/exports/audit-20260101-000000.${format}`;
    return invoke('export_audit_log', { format });
  },

  /** True when on-disk audit persistence has failed — the trail may be incomplete. */
  async isStorageDegraded(): Promise<boolean> {
    if (!isTauri) return false;
    return invoke('audit_storage_degraded');
  },
};
