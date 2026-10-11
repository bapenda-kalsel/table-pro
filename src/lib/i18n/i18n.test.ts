import { describe, expect, it, beforeEach } from 'vitest';
import { en } from './en';
import { id } from './id';
import { getLocale, setLocale, t } from './i18n.svelte';

// Node test env has no Web Storage — provide a minimal stub.
const store = new Map<string, string>();
if (typeof localStorage === 'undefined') {
  (globalThis as any).localStorage = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
    removeItem: (k: string) => void store.delete(k),
  };
}

const enKeys = Object.keys(en);

describe('i18n dictionaries', () => {
  it('id covers every en key', () => {
    for (const k of enKeys) {
      expect(id[k as keyof typeof id], `missing id translation: ${k}`).toBeTruthy();
    }
  });

  it('id has no extra keys', () => {
    for (const k of Object.keys(id)) {
      expect(enKeys, `extra id key: ${k}`).toContain(k);
    }
  });

  it('interpolation placeholders match between en and id', () => {
    for (const k of enKeys) {
      const enParams = (en[k as keyof typeof en].match(/\{\w+\}/g) ?? []).sort();
      const idParams = (id[k as keyof typeof id].match(/\{\w+\}/g) ?? []).sort();
      expect(idParams, `param mismatch on ${k}`).toEqual(enParams);
    }
  });
});

describe('t()', () => {
  beforeEach(() => setLocale('en'));

  it('returns english string in en locale', () => {
    setLocale('en');
    expect(t('common.save')).toBe('Save');
  });

  it('returns indonesian string after setLocale', () => {
    setLocale('id');
    expect(t('common.save')).toBe('Simpan');
    expect(getLocale()).toBe('id');
  });

  it('interpolates {param} placeholders', () => {
    setLocale('id');
    expect(t('common.rows', { count: 42 })).toBe('42 baris');
    setLocale('en');
    expect(t('grid.exported_msg', { count: 5, path: '/tmp/x.csv' })).toBe('Exported 5 rows → /tmp/x.csv');
  });

  it('persists locale to localStorage', () => {
    setLocale('id');
    expect(localStorage.getItem('tablepro.locale')).toBe('id');
    setLocale('en');
  });
});
