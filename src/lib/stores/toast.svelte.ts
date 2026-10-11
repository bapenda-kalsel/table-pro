//! App-wide toast notifications — replaces native alert() popups.

export type ToastKind = 'error' | 'success' | 'info';

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

class ToastStore {
  items = $state<ToastItem[]>([]);
  private seq = 0;

  private push(kind: ToastKind, message: string, timeout: number) {
    const id = ++this.seq;
    this.items.push({ id, kind, message });
    setTimeout(() => this.dismiss(id), timeout);
  }

  error(message: string) {
    this.push('error', message, 7000);
  }

  success(message: string) {
    this.push('success', message, 4000);
  }

  info(message: string) {
    this.push('info', message, 4000);
  }

  dismiss(id: number) {
    this.items = this.items.filter((t) => t.id !== id);
  }
}

export const toasts = new ToastStore();
