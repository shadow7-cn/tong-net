import { create } from "zustand";
import { getServiceStatus, getSettings, startLanService, stopLanService } from "@/api/service";
import { configureDesktopService } from "@/http";
import type { ServiceInfo } from "@/types/domain";

type ServiceState = ServiceInfo & {
  loading: boolean;
  initialize: () => Promise<void>;
  startService: () => Promise<void>;
  stopService: () => Promise<void>;
};

const empty: ServiceInfo = { running: false, port: 7878, lanUrl: "", token: "", tokenRequired: true };

const applyInfo = (info: ServiceInfo) => {
  if (info.running) configureDesktopService(info.port, info.token);
  return { ...info, loading: false };
};

export const useServiceStore = create<ServiceState>((set) => ({
  ...empty,
  loading: false,
  initialize: async () => {
    const [info, settings] = await Promise.all([getServiceStatus(), getSettings()]);
    set(applyInfo(info));
    if (!info.running && settings.autoStartService) {
      set({ loading: true });
      try { set(applyInfo(await startLanService())); } catch (error) { set({ loading: false }); throw error; }
    }
  },
  startService: async () => {
    set({ loading: true });
    try { set(applyInfo(await startLanService())); } catch (error) { set({ loading: false }); throw error; }
  },
  stopService: async () => {
    set({ loading: true });
    try { set(applyInfo(await stopLanService())); } catch (error) { set({ loading: false }); throw error; }
  },
}));
