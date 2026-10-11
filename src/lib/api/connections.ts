import { invoke, isTauri } from './client';
import type { ConnectionProfile } from '../types';

/** Browser-preview store: behaves like connections.json so the UI is fully exercisable without Tauri. */
let mockProfiles: ConnectionProfile[] = [
  {
    id: 'demo-sqlite',
    name: 'Demo SQLite (Local Memory)',
    driver: 'sqlite',
    environment: 'local',
    file_path: ':memory:',
  },
];

export const connectionsApi = {
  async getConnectionProfiles(): Promise<ConnectionProfile[]> {
    if (!isTauri) {
      return mockProfiles.map((p) => ({ ...p }));
    }
    return invoke('get_connection_profiles');
  },

  async saveConnectionProfile(
    profile: ConnectionProfile,
    password?: string,
    sshSecrets?: { sshPassword?: string; sshPassphrase?: string },
  ): Promise<void> {
    if (!isTauri) {
      const idx = mockProfiles.findIndex((p) => p.id === profile.id);
      if (idx >= 0) mockProfiles[idx] = { ...profile };
      else mockProfiles.push({ ...profile });
      return;
    }
    return invoke('save_connection_profile', {
      profile,
      password,
      sshPassword: sshSecrets?.sshPassword ?? null,
      sshPassphrase: sshSecrets?.sshPassphrase ?? null,
    });
  },

  async deleteConnectionProfile(id: string): Promise<void> {
    if (!isTauri) {
      mockProfiles = mockProfiles.filter((p) => p.id !== id);
      return;
    }
    return invoke('delete_connection_profile', { id });
  },

  async testConnection(
    profile: ConnectionProfile,
    password?: string,
    sshSecrets?: { sshPassword?: string; sshPassphrase?: string },
  ): Promise<number> {
    if (!isTauri) return 1.25;
    return invoke('test_connection', {
      profile,
      password: password || null,
      sshPassword: sshSecrets?.sshPassword ?? null,
      sshPassphrase: sshSecrets?.sshPassphrase ?? null,
    });
  },

  /** `password` optionally overrides the keychain for this connect. */
  async connectDatabase(id: string, password?: string): Promise<number> {
    if (!isTauri) return 2.5;
    return invoke('connect_database', { id, password: password || null });
  },

  async disconnectDatabase(id: string): Promise<void> {
    if (!isTauri) return;
    return invoke('disconnect_database', { id });
  },

  async getDataDir(kind: 'exports' | 'backups' | 'config'): Promise<string> {
    if (!isTauri) return `~/.tablepro/${kind}`;
    return invoke('get_data_dir', { kind });
  },
};
