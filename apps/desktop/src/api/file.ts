import { request } from "@/http";

import type { Message } from "@/types/domain";

export function uploadConversationFile(peerDeviceId: string, formData: FormData, onProgress?: (progress: number) => void) {
  return request.post<Message>(`/conversations/${peerDeviceId}/files`, formData, {
    timeout: 0,
    onUploadProgress: (event) => onProgress?.(event.total ? Math.round((event.loaded / event.total) * 100) : 0),
  });
}

export function getDownloadUrl(fileId: string) {
  const token = sessionStorage.getItem("tong-net-token") ?? "";
  const deviceId = sessionStorage.getItem("tong-net-device-id") ?? "";
  const configuredBase = typeof request.defaults.baseURL === "string" && request.defaults.baseURL.startsWith("http")
    ? request.defaults.baseURL.replace(/\/api\/?$/, "")
    : "";
  const path = `/api/files/${fileId}/download?token=${encodeURIComponent(token)}&deviceId=${encodeURIComponent(deviceId)}`;
  return new URL(path, configuredBase || window.location.origin).toString();
}
