import { create } from "zustand";
import type { ToastItem } from "../types";

/// Global toast queue (zustand). `toast.ok/err/info` can be called from
/// anywhere — including non-React code — via the store's static API.
type ToastState = {
  items: ToastItem[];
  push: (kind: ToastItem["kind"], text: string) => void;
  dismiss: (id: number) => void;
};

let seq = 0;

export const useToastStore = create<ToastState>((set) => ({
  items: [],
  push: (kind, text) => {
    const id = ++seq;
    set((s) => ({ items: [...s.items, { id, kind, text }] }));
    setTimeout(() => {
      set((s) => ({ items: s.items.filter((t) => t.id !== id) }));
    }, 5500);
  },
  dismiss: (id) => set((s) => ({ items: s.items.filter((t) => t.id !== id) })),
}));

export const toast = {
  ok: (t: string) => useToastStore.getState().push("ok", t),
  err: (t: string) => useToastStore.getState().push("err", t),
  info: (t: string) => useToastStore.getState().push("info", t),
};
