import { invoke, isTauri } from './client';
import type { ExportResult, GlobalSearchResult } from '../types';

export const opsApi = {
  async runMaintenance(connectionId: string, action: string, table?: string): Promise<string> {
    if (!isTauri) {
      return `Maintenance action '${action}' completed successfully (Mock).`;
    }
    return invoke('run_maintenance', { connectionId, action, table: table || null });
  },

  async searchTextInDatabase(connectionId: string, keyword: string): Promise<GlobalSearchResult[]> {
    if (!isTauri) {
      return [
        {
          table_name: 'users',
          column_name: 'email',
          match_count: 1,
          sample_rows: [{ id: 1, email: 'alice@company.com' }],
        },
      ];
    }
    return invoke('search_text_in_database', { connectionId, keyword });
  },

  async exportTable(
    connectionId: string,
    table: string,
    format: string,
    filter?: string,
  ): Promise<ExportResult> {
    if (!isTauri) {
      return { file_path: `/mock/exports/${table}-demo.${format}`, row_count: 5, byte_size: 420, format };
    }
    return invoke('export_table', { connectionId, table, format, filter: filter || null });
  },

  async importCsv(connectionId: string, table: string, filePath: string): Promise<number> {
    if (!isTauri) return 42;
    return invoke('import_csv', { connectionId, table, filePath });
  },

  async backupDatabase(connectionId: string): Promise<string> {
    if (!isTauri) return '/mock/backups/backup-20260101-000000.db';
    return invoke('backup_database', { connectionId });
  },
};
