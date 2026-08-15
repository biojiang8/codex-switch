//! Codex Switch 配置模型与读写
//! 配置位置：
//!   - Provider 配置:  ~/.codex/codex-switch-config.json
//!   - Codex 配置:     ~/.codex/config.toml
//!   - 认证:           ~/.codex/auth.json
//!   - 环境:           ~/.codex/.env
//! 设计原则（继承 CLI v3.0 教训）：
//!   - config.toml 中每个 provider 常驻独立 [model_providers.XXX] 段（并行保存）
//!   - 切换只把目标 provider 镜像进活跃段 [model_providers.OpenAI]，绝不改段名
//!   - 顶层 model_provider 始终保持 "OpenAI"
//!   - 绝不修改 threads 表的 model_provider（Codex 用它关联段名，改了报错）

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const SWITCH_CONFIG_FILE: &str = "codex-switch-config.json";
pub const CONFIG_TOML_FILE: &str = "config.toml";
pub const AUTH_JSON_FILE: &str = "auth.json";
pub const ENV_FILE: &str = ".env";
pub const STATE_DB_FILE: &str = "state_5.sqlite";
pub const SESSION_INDEX_FILE: &str = "session_index.jsonl";
pub const CACHE_DIR_NAME: &str = "cache";
pub const GLOBAL_STATE_FILE: &str = ".codex-global-state.json";
pub const BACKUPS_DIR: &str = "backups";

pub fn codex_home() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
}

pub fn switch_config_path() -> PathBuf {
    codex_home().join(SWITCH_CONFIG_FILE)
}
pub fn config_toml_path() -> PathBuf {
    codex_home().join(CONFIG_TOML_FILE)
}
pub fn auth_json_path() -> PathBuf {
    codex_home().join(AUTH_JSON_FILE)
}
pub fn env_file_path() -> PathBuf {
    codex_home().join(ENV_FILE)
}
pub fn state_db_path() -> PathBuf {
    codex_home().join(STATE_DB_FILE)
}
pub fn session_index_path() -> PathBuf {
    codex_home().join(SESSION_INDEX_FILE)
}
pub fn cache_dir_path() -> PathBuf {
    codex_home().join(CACHE_DIR_NAME)
}
pub fn global_state_path() -> PathBuf {
    codex_home().join(GLOBAL_STATE_FILE)
}
pub fn backups_dir() -> PathBuf {
    codex_home().join(BACKUPS_DIR)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub display_name: String,
    #[serde(rename = "type")]
    pub ptype: String, // "official" | "custom"
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String, // 留空 = 从 Keychain 读取；desktop 版统一存 Keychain
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_wire_api")]
    pub wire_api: String, // "responses" | "chat"
    #[serde(default)]
    pub catalog_json: String,
    #[serde(default)]
    pub has_keychain_key: bool, // true = Keychain 中已有该 provider 的 key
}

fn default_wire_api() -> String {
    "responses".to_string()
}

impl Default for Provider {
    fn default() -> Self {
        Provider {
            display_name: String::new(),
            ptype: "custom".into(),
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            wire_api: "responses".into(),
            catalog_json: String::new(),
            has_keychain_key: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SwitchConfig {
    pub providers: BTreeMap<String, Provider>,
    #[serde(default)]
    pub default_provider: String,
    #[serde(default = "default_true")]
    pub zshrc_env: bool,
}

fn default_true() -> bool {
    true
}

/// 读取 switch-config.json，不存在时返回默认配置
pub fn load_switch_config() -> SwitchConfig {
    let path = switch_config_path();
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<SwitchConfig>(&content) {
                return cfg;
            }
        }
    }
    SwitchConfig {
        providers: BTreeMap::new(),
        default_provider: "openai".into(),
        zshrc_env: true,
    }
}

/// 保存 switch-config.json（确保目录存在）
pub fn save_switch_config(cfg: &SwitchConfig) -> Result<(), String> {
    let path = switch_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| format!("序列化失败: {e}"))?;
    std::fs::write(&path, json + "\n").map_err(|e| format!("写入失败: {e}"))
}

/// 备份配置三件套（config.toml / auth.json / .env）
pub fn backup_configs() -> Result<String, String> {
    let backup_dir = backups_dir();
    std::fs::create_dir_all(&backup_dir).map_err(|e| format!("创建备份目录失败: {e}"))?;
    let ts = chrono_like_timestamp();
    let mut count = 0;
    for (name, path) in [
        (CONFIG_TOML_FILE, config_toml_path()),
        (AUTH_JSON_FILE, auth_json_path()),
        (ENV_FILE, env_file_path()),
    ] {
        if path.exists() {
            let target = backup_dir.join(format!("{name}.{ts}.bak"));
            if std::fs::copy(&path, &target).is_ok() {
                count += 1;
            }
        }
    }
    Ok(format!("已备份 {count} 个配置文件到 {}", backup_dir.display()))
}

fn chrono_like_timestamp() -> String {
    // 简单时间戳: unix 秒
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
