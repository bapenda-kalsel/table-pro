//! App update check via tauri-plugin-updater. Checks a `latest.json`
//! published on GitHub Releases by the release workflow; artifacts are
//! verified against the minisign pubkey baked into tauri.conf.json.

import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { isTauri } from '$lib/api/client';
import { confirmDialog } from './confirm.svelte';
import { toasts } from './toast.svelte';
import { t } from '$lib/i18n/i18n.svelte';

/// `manual=true` reports "already up to date"/failures to the user;
/// the silent startup call ignores errors (offline is not an error).
export async function checkForUpdates(manual = false) {
  if (!isTauri) {
    if (manual) toasts.info(t('update.desktop_only'));
    return;
  }
  try {
    const update = await check();
    if (!update) {
      if (manual) toasts.success(t('update.latest'));
      return;
    }
    const ok = await confirmDialog.ask({
      title: t('update.available_title'),
      message: t('update.available_msg', { version: update.version }),
      confirmLabel: t('update.install'),
    });
    if (!ok) return;

    await update.downloadAndInstall();
    const restart = await confirmDialog.ask({
      title: t('update.ready_title'),
      message: t('update.ready_msg'),
      confirmLabel: t('update.restart'),
    });
    if (restart) await relaunch();
  } catch (err) {
    if (manual) toasts.error(t('update.failed', { err: String(err) }));
  }
}
