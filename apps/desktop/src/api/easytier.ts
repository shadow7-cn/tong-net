import { invoke } from "@tauri-apps/api/core";

export interface EasyTierConfig {
  serverUrl: string;
  networkName: string;
  networkPassword: string;
  deviceName: string;
  allowInsecureHttp: boolean;
}

export interface EasyTierMember {
  id: string;
  hostname: string;
  ipv4: string;
  cost: string;
  latency: string;
  lossRate: string;
  rxBytes: string;
  txBytes: string;
  protocol: string;
  natType: string;
  version: string;
  local: boolean;
}

export interface EasyTierStatus {
  running: boolean;
  connected: boolean;
  phase: string;
  networkName: string;
  deviceName: string;
  virtualIp: string;
  serverMode: "" | "public" | "private";
  serverUrl: string;
  insecureHttp: boolean;
  members: EasyTierMember[];
  logs: string[];
}

export function getEasyTierStatus() {
  return invoke<EasyTierStatus>("get_easytier_status");
}

export function getEasyTierConfig() {
  return invoke<EasyTierConfig>("get_easytier_config");
}

export function saveEasyTierConfig(config: EasyTierConfig) {
  return invoke<EasyTierConfig>("save_easytier_config", { config });
}

export function startEasyTier(config: EasyTierConfig) {
  return invoke<EasyTierStatus>("start_easytier", { config });
}

export function stopEasyTier() {
  return invoke<EasyTierStatus>("stop_easytier");
}
