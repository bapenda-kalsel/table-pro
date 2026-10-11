export type DatabaseDriverType = 'sqlite' | 'mysql' | 'postgres';
export type EnvironmentTag = 'local' | 'staging' | 'production';

export interface ConnectionProfile {
  id: string;
  name: string;
  driver: DatabaseDriverType;
  environment: EnvironmentTag;
  host?: string;
  port?: number;
  database?: string;
  username?: string;
  file_path?: string;
  ssl_mode?: string;
  created_at?: string;
}

export interface TableInfo {
  name: string;
  table_type: string;
  row_count_estimate?: number;
}

export interface TableColumnMetadata {
  name: string;
  data_type: string;
  is_nullable: boolean;
  is_primary_key: boolean;
  default_value?: string;
  comment?: string;
}

export interface IndexInfo {
  name: string;
  is_unique: boolean;
  origin: string;
  columns: string[];
  is_partial: boolean;
}

export interface ForeignKeyInfo {
  id: number;
  referenced_table: string;
  from_column: string;
  to_column?: string;
  on_update: string;
  on_delete: string;
}

export interface TriggerInfo {
  name: string;
  table_name: string;
  sql: string;
}

export interface FullTableSchema {
  columns: TableColumnMetadata[];
  indexes: IndexInfo[];
  foreign_keys: ForeignKeyInfo[];
  triggers: TriggerInfo[];
  ddl?: string;
}

export interface ExplainStep {
  id: number;
  parent: number;
  detail: string;
  children: ExplainStep[];
}

export interface ExplainResult {
  steps: ExplainStep[];
  warnings: string[];
  has_full_scan: boolean;
  uses_index: boolean;
  uses_temp_btree: boolean;
}

export interface QueryResultData {
  columns: string[];
  column_types: string[];
  rows: any[][];
  execution_time_ms: number;
  affected_rows: number;
  total_rows: number;
}

export interface SqlLogEntry {
  id: string;
  timestamp: string;
  query: string;
  duration_ms: number;
  is_error: boolean;
  error_message?: string;
  connection_id?: string;
  connection_name?: string;
  environment?: string;
  statement_type?: string;
  affected_rows?: number;
}

export interface AuditStats {
  total: number;
  error_count: number;
  avg_duration_ms: number;
  max_duration_ms: number;
  slowest_query?: string;
  by_statement_type: Record<string, number>;
}

export interface ExportResult {
  file_path: string;
  row_count: number;
  byte_size: number;
  format: string;
}

export interface RowUpdatePayload {
  primary_key_values: Record<string, any>;
  updated_fields: Record<string, any>;
}

export interface StagedCommitPayload {
  connection_id: string;
  table_name: string;
  primary_keys: string[];
  inserts: Record<string, any>[];
  updates: RowUpdatePayload[];
  deletes: Record<string, any>[];
}

export interface StagedDiffSummary {
  insert_count: number;
  update_count: number;
  delete_count: number;
  generated_sql: string[];
}

export interface GlobalSearchResult {
  table_name: string;
  column_name: string;
  match_count: number;
  sample_rows: Record<string, any>[];
}

export interface DatabaseStats {
  driver_name: string;
  version: string;
  file_path?: string;
  file_size_bytes: number;
  page_count: number;
  page_size: number;
  freelist_pages: number;
  journal_mode: string;
  encoding: string;
  table_count: number;
  view_count: number;
  index_count: number;
  trigger_count: number;
  total_rows: number;
}

export interface Snippet {
  id: string;
  name: string;
  sql: string;
  created_at: string;
}

export type TabType = 'table' | 'query' | 'structure' | 'audit' | 'erd' | 'dashboard';

export interface WorkspaceTab {
  id: string;
  title: string;
  type: TabType;
  tableName?: string;
  queryText?: string;
  isDirty?: boolean;
}
