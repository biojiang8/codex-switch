//! 状态读取：get_status 命令

use crate::codex::{
    auth_json_path, cache_dir_path, config_toml_path, load_switch_config, state_db_path,
};
use crate::config_toml;
use crate::clean::dir_size_human;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StatusInfo {
    pub current_mode: String,      // "官方订阅" | "自定义 API"
    pub current_display: String,   // 当前 provider 显示名
    pub current_provider: String,  // 当前 provider id
    pub model: String,
    pub base_url: String,
    pub auth_mode: String,
    pub zshrc_has_key: bool,
    pub thread_count: i64,
    pub cache_size: String,
    pub catalog_json: String,
    pub providers: Vec<ProviderStatus>,
    pub codex_installed: bool,
}

#[derive(Debug, Serialize)]
pub struct ProviderStatus {
    pub id: String,
    pub display_name: String,
    pub ptype: String,
    pub model: String,
    pub base_url: String,
    pub has_key: bool,
}

#[tauri::command]
pub fn get_status() -> StatusInfo {
    let cfg = load_switch_config();
    let config_toml = config_toml_path();
    let base_url = config_toml::read_active_base_url(&config_toml);
    let model = config_toml::read_current_model(&config_toml);
    let catalog_json = config_toml::read_current_catalog(&config_toml);

    let current_mode = if base_url.is_empty() {
        "官方订阅".to_string()
    } else {
        "自定义 API".to_string()
    };

    // 反查当前 provider
    let mut current_provider = String::new();
    let mut current_display = current_mode.clone();
    if !base_url.is_empty() {
        let target = base_url.trim_end_matches('/').to_lowercase();
        for (pid, p) in &cfg.providers {
            let bu = p.base_url.trim_end_matches('/').to_lowercase();
            if !bu.is_empty() && bu == target {
                current_provider = pid.clone();
                current_display = p.display_name.clone();
                break;
            }
        }
    }
    if current_provider.is_empty() {
        // 官方模式或反查失败 → 用 default_provider
        current_provider = cfg.default_provider.clone();
        if let Some(p) = cfg.providers.get(&current_provider) {
            if !p.display_name.is_empty() {
                current_display = p.display_name.clone();
            }
        }
    }

    let auth_mode = std::fs::read_to_string(auth_json_path())
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .and_then(|v| v.get("auth_mode").and_then(|a| a.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".into());

    let zshrc_has_key = dirs::home_dir()
        .map(|h| h.join(".zshrc"))
        .map(|z| {
            std::fs::read_to_string(&z)
                .map(|c| c.lines().any(|l| l.starts_with("export OPENAI_API_KEY=")))
                .unwrap_or(false)
        })
        .unwrap_or(false);

    let thread_count = state_db_path()
        .exists()
        .then(|| {
            Connection::open(state_db_path())
                .ok()
                .and_then(|conn| {
                    conn.query_row("SELECT count(*) FROM threads", [], |r| r.get(0))
                        .ok()
                })
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let cache_size = if cache_dir_path().exists() {
        dir_size_human(&cache_dir_path())
    } else {
        "0 B".into()
    };

    let codex_installed = config_toml.exists();

    let mut providers = Vec::new();
    for (pid, p) in &cfg.providers {
        providers.push(ProviderStatus {
            id: pid.clone(),
            display_name: p.display_name.clone(),
            ptype: p.ptype.clone(),
            model: p.model.clone(),
            base_url: p.base_url.clone(),
            has_key: p.has_keychain_key || !p.api_key.is_empty(),
        });
    }

    StatusInfo {
        current_mode,
        current_display,
        current_provider,
        model,
        base_url,
        auth_mode,
        zshrc_has_key,
        thread_count,
        cache_size,
        catalog_json,
        providers,
        codex_installed,
    }
}
