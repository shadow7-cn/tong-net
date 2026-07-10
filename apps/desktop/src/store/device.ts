import { create } from "zustand";
import { listDevices } from "@/api/device";
import type { Device } from "@/types/domain";

type DeviceState = {
  devices: Device[];
  selectedDeviceId: string;
  setSelectedDeviceId: (deviceId: string) => void;
  loadDevices: () => Promise<void>;
  setDevices: (devices: Device[]) => void;
};

export const useDeviceStore = create<DeviceState>((set) => ({
  devices: [],
  selectedDeviceId: "",
  setSelectedDeviceId: (selectedDeviceId) => set({ selectedDeviceId }),
  setDevices: (devices) => set({ devices }),
  loadDevices: async () => set({ devices: (await listDevices()).data }),
}));
