import { ApiError } from './errors';
import { confirmDialog } from '$lib/stores/confirm.svelte';
import { t } from '$lib/i18n/i18n.svelte';

/** Payload carried by a backend `confirmation_required` error. */
export interface ConfirmationChallenge {
  token: string;
  summary: string;
  connection_name: string;
}

export function challengeOf(err: unknown): ConfirmationChallenge | null {
  if (err instanceof ApiError && err.code === 'confirmation_required') {
    const ch = (err.raw as { challenge?: ConfirmationChallenge } | null)?.challenge;
    if (ch?.token) return ch;
  }
  return null;
}

/**
 * Wrap a mutating api call. When the backend enforces a production
 * confirmation it throws `confirmation_required` with a one-shot token;
 * we ask the user once, then replay the SAME call with that token.
 * The token is bound to the exact request + session — there is nothing to
 * retry blindly and a declined prompt propagates the original error.
 */
export async function withProdGuard<T>(fn: (confirmToken?: string) => Promise<T>): Promise<T> {
  try {
    return await fn();
  } catch (err) {
    const ch = challengeOf(err);
    if (ch) {
      const ok = await confirmDialog.ask({
        title: t('guard.title', { name: ch.connection_name }),
        message: ch.summary,
        confirmLabel: t('guard.execute'),
        danger: true,
      });
      if (ok) return await fn(ch.token);
    }
    throw err;
  }
}
