export function resolveLanSocketUrl(pageUrl: string, apiBaseUrl: string | undefined, token: string, deviceId: string) {
  const page = new URL(pageUrl);
  const service = new URL(apiBaseUrl ?? "/api", page);
  const protocol = service.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${service.host}/ws?token=${encodeURIComponent(token)}&deviceId=${encodeURIComponent(deviceId)}`;
}
