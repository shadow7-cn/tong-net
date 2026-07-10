import { request } from "@/http";

import type { Message } from "@/types/domain";

export type UploadProgress = { loaded: number; total: number; progress: number };

export function uploadConversationFile(
  peerDeviceId: string,
  formData: FormData,
  options: { transferId: string; fileName: string; fileSize: number; signal: AbortSignal; onProgress?: (value: UploadProgress) => void },
) {
  return request.post<Message>(`/conversations/${peerDeviceId}/files`, formData, {
    timeout: 0,
    signal: options.signal,
    headers: {
      "X-Transfer-Id": options.transferId,
      "X-File-Name": encodeURIComponent(options.fileName),
      "X-File-Size": String(options.fileSize),
    },
    onUploadProgress: (event) => options.onProgress?.({
      loaded: Math.min(event.loaded, options.fileSize),
      total: options.fileSize,
      progress: options.fileSize ? Math.min(99, Math.round((event.loaded / options.fileSize) * 100)) : 0,
    }),
  });
}

export function cancelTransfer(transferId: string) {
  return request.post(`/transfers/${transferId}/cancel`);
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
