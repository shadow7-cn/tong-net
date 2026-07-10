import axios from "axios";
import { detectClientSource } from "@/utils/clientSource";

const tokenKey = "tong-net-token";
const deviceIdKey = "tong-net-device-id";
const clientIdKey = "tong-net-client-id";
const nicknameKey = "tong-net-device-name";

export function captureAccessToken() {
  const token = new URLSearchParams(window.location.search).get("token");
  if (token) {
    sessionStorage.setItem(tokenKey, token);
    const url = new URL(window.location.href);
    url.searchParams.delete("token");
    window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
  }
  return token ?? sessionStorage.getItem(tokenKey) ?? "";
}

export function setCurrentDeviceId(deviceId: string) {
  sessionStorage.setItem(deviceIdKey, deviceId);
}

export function getAccessToken() {
  return sessionStorage.getItem(tokenKey) ?? "";
}

export function configureDesktopService(port: number, token: string) {
  request.defaults.baseURL = `http://127.0.0.1:${port}/api`;
  sessionStorage.setItem(tokenKey, token);
  setCurrentDeviceId("host");
}

export const request = axios.create({
  baseURL: "/api",
  timeout: 15_000,
});

request.interceptors.request.use((config) => {
  const token = getAccessToken();

  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }

  const deviceId = sessionStorage.getItem(deviceIdKey);
  const clientId = localStorage.getItem(clientIdKey);
  const nickname = localStorage.getItem(nicknameKey);
  if (deviceId) config.headers["X-Device-Id"] = deviceId;
  if (clientId) config.headers["X-Client-Id"] = clientId;
  if (nickname) config.headers["X-Device-Name"] = encodeURIComponent(nickname);
  config.headers["X-Client-Source"] = encodeURIComponent(detectClientSource());

  return config;
});
