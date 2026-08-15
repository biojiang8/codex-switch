//! 切换核心：switch_provider 命令
//! 流程（与 CLI v3.0 对齐）：
//!   1. 读配置 → 2. 备份 → 3. config.toml 切换 → 4. auth.json
//!   → 5. .env → 6. .zshrc（可选）→ 7. SQLite threads.model 同步 → 8. 重启 ChatGPT

use crate::codex::{
    self, auth_json_path, backup_configs, config_toml_path, env_file_path, load_switch_config,
    state_db_path, Provider, SwitchConfig,
};
use crate::config_toml;
use crate::keychain;
use rusqlite::Connection;
use serde::Serialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize)]
pub struct SwitchResult {
    pub ok: bool,
    pub message: String,
    pub steps: Vec<String>,
}

fn step(steps: &mut Vec<String>, msg: String) {
    steps.push(msg);
}

/// 重启 ChatGPT 桌面应用（可通过 skip_restart 跳过）
/// macOS 上自动重启；Windows / Linux 提示手动重启
pub fn restart_chatgpt(skip_restart: bool, steps: &mut Vec<String>) {
    if skip_restart {
        step(steps, "已跳过 ChatGPT 重启".into());
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let running = Command::new("pgrep")
            .args(["-x", "ChatGPT"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if running {
            let _ = Command::new("osascript")
                .args(["-e", r#"tell application "ChatGPT" to quit"#])
                .output();
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        let opened = Command::new("open").args(["-a", "ChatGPT"]).output();
        if opened.map(|o| o.status.success()).unwrap_or(false) {
            step(steps, "ChatGPT 已重启".into());
        } else {
            step(steps, "请手动重启 ChatGPT".into());
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        step(steps, "已切换完成：请手动重启 ChatGPT 应用（或在设置中勾选「跳过重启」）".into());
    }
}

/// 同步 SQLite threads 表的 model / model_provider 字段
/// v4：段名随模式变化（官方="openai"内置，自定义="<pid>"），必须同步 model_provider
fn sync_threads_model(model: &str, provider: &str, steps: &mut Vec<String>) {
    let db_path = state_db_path();
    if !db_path.exists() {
        return;
    }
    match Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    ) {
        Ok(conn) => {
            let _ = conn.execute_batch("PRAGMA busy_timeout=10000;");
            let updated = conn
                .execute(
                    "UPDATE threads SET model = ?1 WHERE model IS NOT NULL AND model != ''",
                    [model],
                )
                .unwrap_or(0);
            step(steps, format!("已同步 {updated} 条会话的 model → {model}"));
            if !provider.is_empty() {
                let updated_mp = conn
                    .execute("UPDATE threads SET model_provider = ?1", [provider])
                    .unwrap_or(0);
                step(steps, format!("已同步 {updated_mp} 条会话的 model_provider → {provider}"));
            }
        }
        Err(e) => {
            step(steps, format!("SQLite 同步跳过: {e}"));
        }
    }
}

/// 写入 auth.json
fn write_auth_json(ptype: &str, api_key: &str, steps: &mut Vec<String>) -> Result<(), String> {
    let mut auth = serde_json::Map::new();
    if ptype == "official" {
        auth.insert("auth_mode".into(), serde_json::Value::String("chatgpt".into()));
        step(steps, "auth.json → ChatGPT 账号模式".into());
    } else {
        auth.insert("auth_mode".into(), serde_json::Value::String("apikey".into()));
        if !api_key.is_empty() {
            auth.insert(
                "OPENAI_API_KEY".into(),
                serde_json::Value::String(api_key.to_string()),
            );
        }
        step(steps, "auth.json → API Key 模式".into());
    }
    let json = serde_json::to_string_pretty(&auth).map_err(|e| e.to_string())?;
    std::fs::write(auth_json_path(), json + "\n").map_err(|e| format!("写入 auth.json 失败: {e}"))
}

/// 写入 .env
fn write_env_file(ptype: &str, api_key: &str, steps: &mut Vec<String>) -> Result<(), String> {
    let content = if ptype == "official" {
        "# API Key removed - using ChatGPT account login\n".to_string()
    } else {
        format!("export OPENAI_API_KEY=\"{api_key}\"\n")
    };
    std::fs::write(env_file_path(), content)
        .map_err(|e| format!("写入 .env 失败: {e}"))?;
    step(
        steps,
        if ptype == "official" {
            ".env → API Key 已清除".into()
        } else {
            ".env → API Key 已写入".into()
        },
    );
    Ok(())
}

/// 更新 ~/.zshrc（zshrc_env 开关）
fn update_zshrc(
    zshrc_env: bool,
    ptype: &str,
    api_key: &str,
    base_url: &str,
    steps: &mut Vec<String>,
) -> Result<(), String> {
    if !zshrc_env {
        return Ok(());
    }
    let zshrc = dirs::home_dir()
        .unwrap_or_else(|| Path::new(".").to_path_buf())
        .join(".zshrc");
    if !zshrc.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&zshrc).unwrap_or_default();
    // 删除旧的 Codex 相关行
    let lines: Vec<String> = content
        .lines()
        .filter(|l| {
            !l.starts_with("# Codex API Configuration")
                && !l.starts_with("export OPENAI_API_KEY=")
                && !l.starts_with("export OPENAI_BASE_URL=")
                && !l.starts_with("# export OPENAI_API_KEY=")
                && !l.starts_with("# export OPENAI_BASE_URL=")
        })
        .map(|s| s.to_string())
        .collect();

    let mut new_lines = lines;
    if ptype != "official" && !api_key.is_empty() && !base_url.is_empty() {
        new_lines.push(String::new());
        new_lines.push("# Codex API Configuration".into());
        new_lines.push(format!("export OPENAI_API_KEY=\"{api_key}\""));
        new_lines.push(format!("export OPENAI_BASE_URL=\"{}/v1\"", base_url.trim_end_matches('/')));
        step(steps, ".zshrc → 环境变量已设置".into());
    } else {
        step(steps, ".zshrc → 环境变量已清除".into());
    }
    std::fs::write(&zshrc, new_lines.join("\n") + "\n")
        .map_err(|e| format!("写入 .zshrc 失败: {e}"))
}

/// 从 Keychain 补齐 provider 的 api_key
fn resolve_api_key(pid: &str, p: &Provider) -> (String, String) {
    // 优先配置内 key（兼容 CLI 时代配置），否则 Keychain
    if !p.api_key.is_empty() {
        return (p.api_key.clone(), "配置内 Key".into());
    }
    let kc = keychain::get_key(pid);
    if !kc.is_empty() {
        return (kc, "Keychain".into());
    }
    (String::new(), "未找到".into())
}

/// 主切换命令
#[tauri::command]
pub fn switch_provider(pid: String, skip_restart: Option<bool>) -> Result<SwitchResult, String> {
    let mut steps: Vec<String> = Vec::new();
    let mut cfg = load_switch_config();

    // 找到目标 provider（大小写不敏感）
    let mut target: Option<(String, Provider)> = None;
    for (k, v) in cfg.providers.iter() {
        if k.eq_ignore_ascii_case(&pid) {
            target = Some((k.clone(), v.clone()));
            break;
        }
    }
    let (target_pid, mut provider) = target.ok_or_else(|| format!("Provider 不存在: {pid}"))?;

    if provider.ptype != "official" {
        let (key, source) = resolve_api_key(&target_pid, &provider);
        if key.is_empty() {
            return Err(format!(
                "Provider「{}」没有 API Key：请在设置中填写或到钥匙串中保存",
                provider.display_name
            ));
        }
        provider.api_key = key;
        step(&mut steps, format!("API Key 已就绪（来源: {source}）"));
    }

    // 备份
    let backup_msg = backup_configs()?;
    step(&mut steps, backup_msg);

    // config.toml 切换
    let all_providers: Vec<(String, Provider)> = cfg
        .providers
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    config_toml::apply_switch(
        &config_toml_path(),
        &target_pid,
        &provider,
        &all_providers,
    )?;
    step(
        &mut steps,
        format!(
            "config.toml → 已切换到 {target_pid}（model={}）",
            provider.model
        ),
    );

    // auth.json
    write_auth_json(&provider.ptype, &provider.api_key, &mut steps)?;

    // .env
    write_env_file(&provider.ptype, &provider.api_key, &mut steps)?;

    // .zshrc
    update_zshrc(
        cfg.zshrc_env,
        &provider.ptype,
        &provider.api_key,
        &provider.base_url,
        &mut steps,
    )?;

    // SQLite 同步（v4：model_provider 也同步，官方→openai，自定义→pid）
    let provider_id = if provider.ptype == "official" {
        "openai"
    } else {
        &target_pid
    };
    sync_threads_model(&provider.model, provider_id, &mut steps);

    // 重启 ChatGPT
    restart_chatgpt(skip_restart.unwrap_or(false), &mut steps);

    // 更新 default_provider
    cfg.default_provider = target_pid.clone();
    let _ = codex::save_switch_config(&cfg);

    Ok(SwitchResult {
        ok: true,
        message: format!("已切换到 {}", provider.display_name),
        steps,
    })
}

/// 获取当前配置（供前端展示）
#[tauri::command]
pub fn get_config() -> Result<SwitchConfig, String> {
    let mut cfg = load_switch_config();
    // 标记 Keychain 中是否有 key
    for (pid, p) in cfg.providers.iter_mut() {
        if p.ptype == "custom" && p.api_key.is_empty() {
            p.has_keychain_key = !keychain::get_key(pid).is_empty();
        }
    }
    Ok(cfg)
}

/// 保存 Provider（新增或更新）
#[tauri::command]
pub fn save_provider(pid: String, provider: Provider) -> Result<(), String> {
    let mut cfg = load_switch_config();
    let mut p = provider;
    // Key 处理：非空时写入 Keychain，配置内不留明文
    if p.ptype == "custom" && !p.api_key.is_empty() {
        keychain::set_key(&pid, &p.api_key)?;
        p.has_keychain_key = true;
    }
    p.api_key = String::new(); // 配置内永不存明文（desktop 版）
    cfg.providers.insert(pid, p);
    codex::save_switch_config(&cfg)
}

/// 删除 Provider（含 Keychain 条目）
#[tauri::command]
pub fn delete_provider(pid: String) -> Result<(), String> {
    let mut cfg = load_switch_config();
    cfg.providers.remove(&pid);
    let _ = keychain::delete_key(&pid);
    codex::save_switch_config(&cfg)
}

/// 更新通用设置
#[tauri::command]
pub fn save_general_settings(
    default_provider: String,
    zshrc_env: bool,
) -> Result<(), String> {
    let mut cfg = load_switch_config();
    cfg.default_provider = default_provider;
    cfg.zshrc_env = zshrc_env;
    codex::save_switch_config(&cfg)
}

/// 测试连接：请求 {base_url}/v1/models（带 Authorization）
#[tauri::command]
pub fn test_connection(
    base_url: String,
    api_key: String,
    timeout_secs: Option<u64>,
) -> Result<serde_json::Value, String> {
    let base = base_url.trim_end_matches('/').to_string();
    let url = format!("{base}/v1/models");
    let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(10));
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    let resp = agent
        .get(&url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .call()
        .map_err(|e| format!("连接失败: {e}"))?;
    let body = resp
        .into_string()
        .map_err(|e| format!("读取响应失败: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    let models: Vec<String> = json
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    Ok(serde_json::json!({
        "ok": true,
        "model_count": models.len(),
        "models": models,
    }))
}
