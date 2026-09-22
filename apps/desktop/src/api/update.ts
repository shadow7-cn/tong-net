import { invoke } from "@tauri-apps/api/core";

export interface UpdateInfo {
  currentVersion: string;
  latestVersion: string;
  releaseUrl: string;
  notes: string;
  available: boolean;
}

export const checkForUpdates = () => invoke<UpdateInfo>("check_for_updates");
