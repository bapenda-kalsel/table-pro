import { invoke, isTauri } from './client';
import type { Snippet } from '../types';

export const snippetsApi = {
  async listSnippets(): Promise<Snippet[]> {
    if (!isTauri) {
      return [
        { id: 's1', name: 'All users', sql: 'SELECT * FROM users LIMIT 50;', created_at: '2026-10-08' },
        { id: 's2', name: 'Active count', sql: "SELECT COUNT(*) FROM users WHERE is_active = 1;", created_at: '2026-10-08' },
      ];
    }
    return invoke('list_snippets');
  },

  async saveSnippet(snippet: Snippet): Promise<void> {
    if (!isTauri) return;
    return invoke('save_snippet', { snippet });
  },

  async deleteSnippet(id: string): Promise<void> {
    if (!isTauri) return;
    return invoke('delete_snippet', { id });
  },
};
