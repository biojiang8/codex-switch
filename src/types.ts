// 与 Rust 后端对应的类型定义

export interface Provider {
  display_name: string;
  type: "official" | "custom";
  base_url: string;
  api_key: string;
  model: string;
  wire_api: "responses" | "chat";
  catalog_json: string;
  has_keychain_key?: boolean;
}

export interface SwitchConfig {
  providers: Record<string, Provider>;
  default_provider: string;
  zshrc_env: boolean;
}

export interface ProviderStatus {
  id: string;
  display_name: string;
  ptype: string;
  model: string;
  base_url: string;
  has_key: boolean;
}

export interface StatusInfo {
  current_mode: string;
  current_display: string;
  current_provider: string;
  model: string;
  base_url: string;
  auth_mode: string;
  zshrc_has_key: boolean;
  thread_count: number;
  cache_size: string;
  catalog_json: string;
  providers: ProviderStatus[];
  codex_installed: boolean;
}

export interface SwitchResult {
  ok: boolean;
  message: string;
  steps: string[];
}

export interface CleanResult {
  ok: boolean;
  steps: string[];
}

export interface KeychainStatus {
  available: boolean;
  message: string;
}

export interface TestConnectionResult {
  ok: boolean;
  model_count: number;
  models: string[];
}
