import { create } from "zustand";

/** A pending passphrase prompt. `confirm` asks for the value twice, which is
 *  what you want when the passphrase is being set rather than entered. */
export type PassphraseReq = {
  title: string;
  message: string;
  confirm: boolean;
  resolve: (value: string | null) => void;
};

type PassphraseState = {
  req: PassphraseReq | null;
  ask: (req: PassphraseReq) => void;
  clear: () => void;
};

export const usePassphraseStore = create<PassphraseState>((set) => ({
  req: null,
  ask: (req) => set({ req }),
  clear: () => set({ req: null }),
}));

/**
 * Prompt for an encryption passphrase. Resolves to null when cancelled.
 *
 * The value is handed straight to the backend and never stored: it is the
 * only thing standing between the server's ciphertext and the profile.
 */
export function passphraseModal(opts: {
  title: string;
  message: string;
  confirm?: boolean;
}): Promise<string | null> {
  return new Promise((resolve) => {
    usePassphraseStore.getState().ask({
      title: opts.title,
      message: opts.message,
      confirm: opts.confirm ?? false,
      resolve: (v) => {
        usePassphraseStore.getState().clear();
        resolve(v);
      },
    });
  });
}
