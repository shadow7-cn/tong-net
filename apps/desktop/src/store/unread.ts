import { create } from "zustand";
import { listMessages } from "@/api/message";

type UnreadState = {
  identityId: string;
  count: number;
  initialized: boolean;
  cursor?: string;
  visible: boolean;
  configure: (identityId: string) => void;
  setVisible: (visible: boolean) => void;
  sync: () => Promise<void>;
};

const storageKey = (id: string) => `tong-net-group-unread:${id}`;
let pending: Promise<void> | undefined;
let requested = false;

export const useUnreadStore = create<UnreadState>((set, get) => ({
  identityId: "",
  count: 0,
  initialized: false,
  visible: false,
  configure: (identityId) => {
    if (get().identityId === identityId) return;
    let saved: { count?: number; cursor?: string; initialized?: boolean } = {};
    try { saved = JSON.parse(localStorage.getItem(storageKey(identityId)) ?? "{}"); } catch { /* Optional persistence. */ }
    set({ identityId, count: saved.count ?? 0, cursor: saved.cursor, initialized: saved.initialized ?? false, visible: false });
  },
  setVisible: (visible) => {
    set({ visible, ...(visible ? { count: 0 } : {}) });
    const state = get();
    try { localStorage.setItem(storageKey(state.identityId), JSON.stringify({ count: state.count, cursor: state.cursor, initialized: state.initialized })); } catch { /* Optional persistence. */ }
  },
  sync: () => {
    requested = true;
    if (pending) return pending;
    pending = (async () => {
      while (requested) {
        requested = false;
        const { identityId, cursor } = get();
        if (!identityId) return;
        const { data } = await listMessages(cursor ? { after: cursor } : undefined);
        if (get().identityId !== identityId) { requested = true; continue; }
        const state = get();
        const count = state.visible ? 0 : state.count + (state.initialized ? data.filter((item) => item.fromDeviceId !== identityId && item.type !== "system").length : 0);
        const nextCursor = data[data.length - 1]?.id ?? cursor ?? "0";
        set({ count, cursor: nextCursor, initialized: true });
        try { localStorage.setItem(storageKey(identityId), JSON.stringify({ count, cursor: nextCursor, initialized: true })); } catch { /* Optional persistence. */ }
        if (cursor && data.length === 50) requested = true;
      }
    })().finally(() => { pending = undefined; });
    return pending;
  },
}));
