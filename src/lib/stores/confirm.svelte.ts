//! Promise-based in-app confirm dialog — replaces native confirm().

export interface ConfirmRequest {
  title: string;
  message?: string;
  confirmLabel?: string;
  danger?: boolean;
}

interface PendingConfirm extends ConfirmRequest {
  resolve: (ok: boolean) => void;
}

class ConfirmStore {
  current = $state<PendingConfirm | null>(null);

  ask(req: ConfirmRequest): Promise<boolean> {
    // A second confirm while one is open resolves the older one as cancelled.
    this.current?.resolve(false);
    return new Promise((resolve) => {
      this.current = { ...req, resolve };
    });
  }

  answer(ok: boolean) {
    this.current?.resolve(ok);
    this.current = null;
  }
}

export const confirmDialog = new ConfirmStore();
