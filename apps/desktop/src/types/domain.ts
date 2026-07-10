export type DeviceKind = "host" | "browser";

export type DeviceStatus = "online" | "offline";

export type Device = {
  id: string;
  name: string;
  kind: DeviceKind;
  status: DeviceStatus;
  browserSource: string;
  removed: boolean;
  lastSeenAt: string;
};

export type MessageType = "text" | "file" | "system";

export type FileRecord = {
  id: string;
  name: string;
  size: number;
  status: "available" | "failed";
  createdAt: string;
};

export type Message = {
  id: string;
  conversationId: string;
  fromDeviceId: string;
  toDeviceId: string;
  type: MessageType;
  content: string;
  file?: FileRecord;
  createdAt: string;
};

export type Conversation = {
  id: string;
  deviceAId: string;
  deviceBId: string;
  updatedAt: string;
};

export type TransferTask = {
  id: string;
  kind: "upload" | "download";
  fileName: string;
  peerName: string;
  progress: number;
  status: "running" | "success" | "failed" | "canceled";
  createdAt?: string;
  totalBytes?: number;
  transferredBytes?: number;
  finishedAt?: string;
};

export type ServiceInfo = {
  running: boolean;
  port: number;
  lanUrl: string;
  token: string;
  startedAt?: string;
};

export type AppSettings = {
  hostName: string;
  port: number;
  saveDir: string;
  rotateToken: boolean;
  cleanupTemp: boolean;
};
