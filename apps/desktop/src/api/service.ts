import { invoke } from "@tauri-apps/api/core";
import { request } from "@/http";
import type { AppSettings, Device, ServiceInfo } from "@/types/domain";

export type BootstrapResponse = {
  serviceName: string;
  hostDeviceId: string;
  currentDevice: Device;
};

export function getBootstrap() {
  return request.get<BootstrapResponse>("/bootstrap");
}

export const isTauri = () => "__TAURI_INTERNALS__" in window;
export const getServiceStatus = () => invoke<ServiceInfo>("get_service_status");
export const startLanService = () => invoke<ServiceInfo>("start_service");
export const stopLanService = () => invoke<ServiceInfo>("stop_service");
export const getSettings = () => invoke<AppSettings>("get_settings");
export const updateSettings = (settings: AppSettings) => invoke<AppSettings>("update_settings", { settings });
export const openSaveDirectory = () => invoke<void>("open_save_directory");
