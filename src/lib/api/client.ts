import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { normalizeError } from './errors';

/** True only inside the real Tauri webview; false in plain browser (mock mode). */
export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/**
 * Invoke wrapper: rejects with a normalized `ApiError` instead of the raw
 * payload (which may be a string or a tagged `{code, message}` object).
 * No automatic retries — retrying mutations can double-execute them.
 */
export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (err) {
    throw normalizeError(err);
  }
}
