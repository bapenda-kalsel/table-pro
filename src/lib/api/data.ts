import { invoke, isTauri } from './client';
import type {
  ExplainResult,
  QueryResultData,
  StagedCommitPayload,
  StagedDiffSummary,
} from '../types';

export const dataApi = {
  async getTableData(
    connectionId: string,
    table: string,
    limit: number = 100,
    offset: number = 0,
    filter?: string,
    sortBy?: string,
    sortDir?: string,
  ): Promise<QueryResultData> {
    if (!isTauri) {
      return {
        columns: ['id', 'name', 'email', 'role', 'created_at'],
        column_types: ['INTEGER', 'TEXT', 'TEXT', 'TEXT', 'DATETIME'],
        rows: [
          [1, 'Alice Smith', 'alice@company.com', 'admin', '2026-01-15 08:30:00'],
          [2, 'Bob Johnson', 'bob@domain.org', 'editor', '2026-02-20 14:12:00'],
          [3, 'Charlie Brown', 'charlie@gmail.com', 'user', '2026-03-01 09:45:00'],
          [4, 'Diana Prince', 'diana@amazon.com', 'user', '2026-03-18 11:20:00'],
          [5, 'Evan Wright', 'evan@tech.io', 'viewer', '2026-04-05 16:50:00'],
        ],
        execution_time_ms: 3.42,
        affected_rows: 5,
        total_rows: 5,
      };
    }
    return invoke('get_table_data', {
      connectionId,
      query: {
        table,
        limit,
        offset,
        filter: filter || null,
        sort_by: sortBy || null,
        sort_dir: sortDir || null,
      },
    });
  },

  async executeQuery(
    connectionId: string,
    sql: string,
    confirmToken?: string,
  ): Promise<QueryResultData> {
    if (!isTauri) {
      return {
        columns: ['result'],
        column_types: ['TEXT'],
        rows: [['Query executed in mock mode: ' + sql]],
        execution_time_ms: 1.5,
        affected_rows: 1,
        total_rows: 1,
      };
    }
    return invoke('execute_query', { connectionId, sql, confirmToken: confirmToken ?? null });
  },

  async explainQuery(connectionId: string, sql: string): Promise<ExplainResult> {
    if (!isTauri) {
      return {
        steps: [
          { id: 0, parent: 0, detail: 'SCAN users', children: [] },
        ],
        warnings: ["Full table scan: 'SCAN users' — consider adding an index on the filtered columns."],
        has_full_scan: true,
        uses_index: false,
        uses_temp_btree: false,
      };
    }
    return invoke('explain_query', { connectionId, sql });
  },

  async previewStagedChanges(payload: StagedCommitPayload): Promise<StagedDiffSummary> {
    if (!isTauri) {
      return {
        insert_count: payload.inserts.length,
        update_count: payload.updates.length,
        delete_count: payload.deletes.length,
        generated_sql: ['-- Mock preview SQL statements'],
      };
    }
    return invoke('preview_staged_changes', { payload });
  },

  async commitStagedChanges(
    payload: StagedCommitPayload,
    confirmToken?: string,
  ): Promise<QueryResultData> {
    if (!isTauri) {
      return {
        columns: ['Committed Statements', 'Total Affected'],
        column_types: ['INTEGER', 'INTEGER'],
        rows: [[1, 1]],
        execution_time_ms: 2.1,
        affected_rows: 1,
        total_rows: 1,
      };
    }
    return invoke('commit_staged_changes', { payload, confirmToken: confirmToken ?? null });
  },
};
