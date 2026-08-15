// Tauri invoke 封装
import { invoke } from "@tauri-apps/api/core";
import type {
  CleanResult,
  KeychainStatus,
  Provider,
  StatusInfo,
  SwitchConfig,
  SwitchResult,
  TestConnectionResult,
} from "./types";

export const api = {
  getStatus: () => invoke<StatusInfo>("get_status"),
  getConfig: () => invoke<SwitchConfig>("get_config"),
  switchProvider: (pid: string, skipRestart = false) =>
    invoke<SwitchResult>("switch_provider", { pid, skipRestart }),
  saveProvider: (pid: string, provider: Provider) =>
    invoke<void>("save_provider", { pid, provider }),
  deleteProvider: (pid: string) => invoke<void>("delete_provider", { pid }),
  saveGeneralSettings: (defaultProvider: string, zshrcEnv: boolean) =>
    invoke<void>("save_general_settings", { defaultProvider, zshrcEnv }),
  testConnection: (baseUrl: string, apiKey: string) =>
    invoke<TestConnectionResult>("test_connection", { baseUrl, apiKey }),
  cleanCache: (deep = false, skipRestart = false) =>
    invoke<CleanResult>("clean_cache", { deep, skipRestart }),
  keychainAvailable: () => invoke<KeychainStatus>("is_available"),
};
