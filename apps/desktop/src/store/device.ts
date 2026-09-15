import { create } from "zustand";
import { listDevices } from "@/api/device";
import type { Device } from "@/types/domain";

type DeviceState = {
  devices: Device[];
  loadDevices: () => Promise<void>;
  setDevices: (devices: Device[]) => void;
};

export const useDeviceStore = create<DeviceState>((set) => ({
  devices: [],
  setDevices: (devices) => set({ devices }),
  loadDevices: async () => set({ devices: (await listDevices()).data }),
}));
