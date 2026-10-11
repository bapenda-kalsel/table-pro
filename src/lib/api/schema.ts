import { invoke, isTauri } from './client';
import type {
  DatabaseStats,
  FullTableSchema,
  TableColumnMetadata,
  TableInfo,
} from '../types';

export const schemaApi = {
  async listTables(connectionId: string): Promise<TableInfo[]> {
    if (!isTauri) {
      return [
        { name: 'users', table_type: 'table', row_count_estimate: 5 },
        { name: 'orders', table_type: 'table', row_count_estimate: 12 },
        { name: 'products', table_type: 'table', row_count_estimate: 8 },
      ];
    }
    return invoke('list_tables', { connectionId });
  },

  async getTableSchema(connectionId: string, table: string): Promise<TableColumnMetadata[]> {
    if (!isTauri) {
      return [
        { name: 'id', data_type: 'INTEGER', is_nullable: false, is_primary_key: true },
        { name: 'name', data_type: 'TEXT', is_nullable: false, is_primary_key: false },
        { name: 'email', data_type: 'TEXT', is_nullable: true, is_primary_key: false },
        { name: 'role', data_type: 'TEXT', is_nullable: false, is_primary_key: false, default_value: "'user'" },
        { name: 'created_at', data_type: 'DATETIME', is_nullable: true, is_primary_key: false },
      ];
    }
    return invoke('get_table_schema', { connectionId, table });
  },

  async getTableDdl(connectionId: string, table: string): Promise<string> {
    if (!isTauri) {
      return `CREATE TABLE "${table}" (\n  "id" INTEGER PRIMARY KEY AUTOINCREMENT,\n  "name" TEXT NOT NULL\n);`;
    }
    return invoke('get_table_ddl', { connectionId, table });
  },

  async getFullSchema(connectionId: string, table: string): Promise<FullTableSchema> {
    if (!isTauri) {
      return {
        columns: [
          { name: 'id', data_type: 'INTEGER', is_nullable: false, is_primary_key: true },
          { name: 'name', data_type: 'TEXT', is_nullable: false, is_primary_key: false },
        ],
        indexes: [
          { name: 'idx_users_email', is_unique: true, origin: 'c', columns: ['email'], is_partial: false },
        ],
        foreign_keys: [],
        triggers: [],
        ddl: `CREATE TABLE "${table}" (\n  "id" INTEGER PRIMARY KEY AUTOINCREMENT,\n  "name" TEXT NOT NULL\n);`,
      };
    }
    return invoke('get_full_schema', { connectionId, table });
  },

  async getServerVersion(connectionId: string): Promise<string> {
    if (!isTauri) return 'SQLite 3.46.0 (mock)';
    return invoke('get_server_version', { connectionId });
  },

  async getDatabaseStats(connectionId: string): Promise<DatabaseStats> {
    if (!isTauri) {
      return {
        driver_name: 'SQLite',
        version: 'SQLite 3.46.0',
        file_path: ':memory:',
        file_size_bytes: 53248,
        page_count: 13,
        page_size: 4096,
        freelist_pages: 0,
        journal_mode: 'wal',
        encoding: 'UTF-8',
        table_count: 3,
        view_count: 0,
        index_count: 2,
        trigger_count: 0,
        total_rows: 25,
      };
    }
    return invoke('get_database_stats', { connectionId });
  },

  /** phpMyAdmin-style visual relation: add a FOREIGN KEY between two columns. */
  async addForeignKey(params: {
    connectionId: string;
    constraintName: string;
    table: string;
    column: string;
    refTable: string;
    refColumn: string;
    onDelete: string;
    onUpdate: string;
  }): Promise<string> {
    if (!isTauri) {
      return `Executed: ALTER TABLE "${params.table}" ADD CONSTRAINT "${params.constraintName}" FOREIGN KEY ("${params.column}") REFERENCES "${params.refTable}"("${params.refColumn}") ON DELETE ${params.onDelete} ON UPDATE ${params.onUpdate};`;
    }
    return invoke('add_foreign_key', params);
  },
};
