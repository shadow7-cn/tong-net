import { create } from "zustand";
import {
  getEasyTierStatus,
  startEasyTier,
  stopEasyTier,
  type EasyTierConfig,
  type EasyTierStatus,
} from "@/api/easytier";

type EasyTierState = EasyTierStatus & {
  loading: boolean;
  refresh: () => Promise<void>;
  connect: (config: EasyTierConfig) => Promise<void>;
  disconnect: () => Promise<void>;
};

const empty: EasyTierStatus = {
  running: false,
  connected: false,
  phase: "未连接",
  networkName: "",
  deviceName: "",
  virtualIp: "",
  members: [],
  logs: [],
};

export const useEasyTierStore = create<EasyTierState>((set) => ({
  ...empty,
  loading: false,
  refresh: async () => set(await getEasyTierStatus()),
  connect: async (config) => {
    set({ loading: true });
    try {
      set({ ...(await startEasyTier(config)), loading: false });
    } catch (error) {
      set({ loading: false });
      throw error;
    }
  },
  disconnect: async () => {
    set({ loading: true });
    try {
      set({ ...(await stopEasyTier()), loading: false });
    } catch (error) {
      set({ loading: false });
      throw error;
    }
  },
}));
