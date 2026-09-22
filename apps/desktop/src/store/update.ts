import { create } from "zustand";
import { getVersion } from "@tauri-apps/api/app";
import { isTauri } from "@/api/service";
import { checkForUpdates, type UpdateInfo } from "@/api/update";

interface UpdateState {
  currentVersion: string;
  checking: boolean;
  availableUpdate: UpdateInfo | null;
  start: () => void;
  check: () => Promise<UpdateInfo>;
  dismiss: () => void;
}

let started = false;
let pending: Promise<UpdateInfo> | null = null;

export const useUpdateStore = create<UpdateState>((set, get) => ({
  currentVersion: "",
  checking: false,
  availableUpdate: null,
  start: () => {
    if (!isTauri() || started) return;
    started = true;
    void getVersion().then((currentVersion) => set({ currentVersion })).catch(() => {});
    void get().check().catch(() => {});
  },
  check: () => {
    if (pending) return pending;
    set({ checking: true });
    pending = checkForUpdates().then((result) => {
      set({ currentVersion: result.currentVersion, availableUpdate: result.available ? result : null });
      return result;
    }).finally(() => {
      pending = null;
      set({ checking: false });
    });
    return pending;
  },
  dismiss: () => set({ availableUpdate: null }),
}));
