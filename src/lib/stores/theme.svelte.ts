//! Light/dark theme — semantic tokens in app.css read `data-theme` on
//! <html>. Choice is persisted to localStorage so it survives restarts
//! without touching the vault.

export type Theme = 'dark' | 'light';

const STORAGE_KEY = 'tablepro-theme';

class ThemeStore {
  current = $state<Theme>('dark');

  constructor() {
    if (typeof window !== 'undefined') {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved === 'light' || saved === 'dark') this.current = saved;
      this.apply(this.current);
    }
  }

  private apply(theme: Theme) {
    if (typeof document !== 'undefined') {
      document.documentElement.dataset.theme = theme;
    }
  }

  set(theme: Theme) {
    this.current = theme;
    this.apply(theme);
    localStorage.setItem(STORAGE_KEY, theme);
  }

  toggle() {
    this.set(this.current === 'dark' ? 'light' : 'dark');
  }
}

export const theme = new ThemeStore();
