import { request } from "@/http";
import type { Device } from "@/types/domain";

export function listDevices() {
  return request.get<Device[]>("/devices");
}

export function updateDeviceName(name: string) {
  return request.patch<Device>("/devices/me", { name });
}

export function removeDevice(deviceId: string) {
  return request.delete(`/devices/${deviceId}`);
}

export function saveDeviceName(name: string) {
  localStorage.setItem("tong-net-device-name", name);
}
