import { invoke } from '@tauri-apps/api/core';

/** True only inside the real Tauri webview; false in plain browser (mock mode). */
export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export { invoke };
