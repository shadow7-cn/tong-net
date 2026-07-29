export interface PublicInfo {
  initialized: boolean;
  siteName: string;
  mode: "public" | "private";
  version: string;
  minimumDesktopVersion: string;
  publicHost: string;
  webPort: number;
  easytierPort: number;
  sharedPublicKey: string;
}

export interface Overview {
  siteName: string;
  mode: "public" | "private";
  version: string;
  webPort: number;
  easytierPort: number;
  networkCount: number;
  deviceCount: number;
  onlineCount: number;
  easytier: {
    running: boolean;
    healthy: boolean;
    managerTotal: number;
    managerRunning: number;
    lastError: string;
  };
  publicMembers: Member[];
}

export interface Network {
  id: string;
  name: string;
  status: "active" | "disabled";
  slot: number;
  deviceCount: number;
  onlineCount: number;
  createdAt: string;
}

export interface Member {
  membershipId?: string;
  networkId?: string;
  networkName?: string;
  name?: string;
  adminNote?: string;
  platform?: string;
  clientVersion?: string;
  status?: "active" | "revoked";
  online?: boolean;
  virtualIp?: string;
  protocol?: string;
  latencyMs?: number;
  rxBytes?: number | string;
  txBytes?: number | string;
  lastSeenAt?: string;
  id?: string;
  hostname?: string;
  ipv4?: string;
  latency?: string;
  version?: string;
}

export interface Settings {
  siteName: string;
  publicHost: string;
  mode: "public" | "private";
  adminUsername: string;
  webPort: number;
  easytierPort: number;
  version: string;
  easytierVersion: string;
}

export interface AuditResult {
  items: Array<{
    id: string;
    actorType: string;
    action: string;
    targetType: string;
    targetId: string;
    result: string;
    ipAddress: string;
    metadata: Record<string, unknown>;
    createdAt: string;
  }>;
  total: number;
  page: number;
  pageSize: number;
}

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
  ) {
    super(message);
  }
}

export async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    credentials: "same-origin",
    ...init,
    headers: {
      ...(init?.body ? { "Content-Type": "application/json" } : {}),
      ...init?.headers,
    },
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new ApiError(response.status, payload.code ?? "REQUEST_FAILED", payload.message ?? "请求失败");
  }
  return payload as T;
}

export const api = {
  info: () => request<PublicInfo>("/api/v1/info"),
  setup: (payload: unknown) =>
    request<{ ok: true }>("/api/v1/setup", { method: "POST", body: JSON.stringify(payload) }),
  login: (payload: unknown) =>
    request<{ ok: true }>("/api/v1/admin/login", {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  logout: () => request("/api/v1/admin/logout", { method: "POST" }),
  overview: () => request<Overview>("/api/v1/admin/overview"),
  networks: () => request<Network[]>("/api/v1/admin/networks"),
  createNetwork: (payload: unknown) =>
    request("/api/v1/admin/networks", { method: "POST", body: JSON.stringify(payload) }),
  networkAction: (id: string, action: "enable" | "disable", payload: unknown = {}) =>
    request(`/api/v1/admin/networks/${id}/${action}`, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  resetNetworkPassword: (id: string, payload: unknown) =>
    request(`/api/v1/admin/networks/${id}/reset-password`, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  deleteNetwork: (id: string) =>
    request(`/api/v1/admin/networks/${id}`, { method: "DELETE" }),
  devices: (networkId?: string) =>
    request<{ mode: "public" | "private"; members: Member[] }>(
      `/api/v1/admin/devices${networkId ? `?network_id=${encodeURIComponent(networkId)}` : ""}`,
    ),
  updateMembership: (id: string, payload: unknown) =>
    request(`/api/v1/admin/memberships/${id}`, {
      method: "PATCH",
      body: JSON.stringify(payload),
    }),
  revokeMembership: (id: string) =>
    request(`/api/v1/admin/memberships/${id}/revoke`, {
      method: "POST",
      body: JSON.stringify({}),
    }),
  deleteMembership: (id: string) =>
    request(`/api/v1/admin/memberships/${id}`, { method: "DELETE" }),
  audit: (page: number, pageSize: number) =>
    request<AuditResult>(`/api/v1/admin/audit-logs?page=${page}&page_size=${pageSize}`),
  clearAudit: () => request("/api/v1/admin/audit-logs", { method: "DELETE" }),
  settings: () => request<Settings>("/api/v1/admin/settings"),
  updateSettings: (payload: unknown) =>
    request<{ ok: true; reauthRequired: boolean }>("/api/v1/admin/settings", {
      method: "PATCH",
      body: JSON.stringify(payload),
    }),
  changeMode: (payload: unknown) =>
    request("/api/v1/admin/mode", { method: "POST", body: JSON.stringify(payload) }),
  retryEasyTier: () =>
    request("/api/v1/admin/easytier/retry", {
      method: "POST",
      body: JSON.stringify({}),
    }),
};
