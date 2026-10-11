import { en, type MessageKey } from './en';
import { id } from './id';

export type Locale = 'en' | 'id';

const dicts: Record<Locale, Record<MessageKey, string>> = { en, id };

const STORAGE_KEY = 'tablepro.locale';

function detectLocale(): Locale {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === 'en' || saved === 'id') return saved;
    if (typeof navigator !== 'undefined' && navigator.language?.toLowerCase().startsWith('id')) {
      return 'id';
    }
  } catch {
    // storage/blocked contexts -> default
  }
  return 'en';
}

let locale = $state<Locale>(detectLocale());

export function getLocale(): Locale {
  return locale;
}

export function setLocale(l: Locale) {
  locale = l;
  try {
    localStorage.setItem(STORAGE_KEY, l);
  } catch {
    // best-effort persistence
  }
}

export const locales: { code: Locale; label: string; flag: string }[] = [
  { code: 'en', label: 'English', flag: 'EN' },
  { code: 'id', label: 'Bahasa Indonesia', flag: 'ID' },
];

/**
 * Translate a message key. `{param}` placeholders are interpolated.
 * Falls back to English, then to the key itself — never crashes the UI.
 */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  let s = dicts[locale][key] ?? dicts.en[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      s = s.replaceAll(`{${k}}`, String(v));
    }
  }
  return s;
}

export type { MessageKey };
