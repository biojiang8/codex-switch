//! config.toml 编辑核心（v4 架构）
//! v4 架构（2026-08-15，适配新版 ChatGPT 应用 26.810+）：
//!   - 新版 Codex 将 `openai` 设为保留内置 provider ID，禁止自定义覆盖
//!   - 因此不再有"镜像进 [model_providers.OpenAI] 活跃段"的架构
//!   - 官方模式：顶层 model_provider = "openai"（内置），不写任何自定义段
//!   - 自定义模式：顶层 model_provider = "<provider id>"，段名与 id 一致
//!   - threads.model_provider 必须随切换同步（段名随模式变化）

use crate::codex::Provider;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table, Value};

fn value_str(s: &str) -> Item {
    Item::Value(Value::from(s.to_string()))
}
fn value_bool(b: bool) -> Item {
    Item::Value(Value::from(b))
}

/// 由 provider 生成段内容（不含段头）
fn build_provider_table(pid: &str, p: &Provider) -> Table {
    let mut table = Table::new();
    let name = if p.ptype == "official" {
        "OpenAI".to_string()
    } else if !p.display_name.is_empty() {
        p.display_name.clone()
    } else {
        pid.to_string()
    };
    table["name"] = value_str(&name);
    table["wire_api"] = value_str(if p.wire_api.is_empty() {
        "responses"
    } else {
        &p.wire_api
    });
    if p.ptype == "official" {
        table["requires_openai_auth"] = value_bool(true);
    } else {
        table["requires_openai_auth"] = value_bool(false);
        if !p.base_url.is_empty() {
            table["base_url"] = value_str(&p.base_url);
        }
        if !p.api_key.is_empty() {
            // 新版 Codex 只认 experimental_bearer_token（api_key 字段已废弃）
            table["experimental_bearer_token"] = value_str(&p.api_key);
        }
    }
    table
}

/// 确保 model_providers 表存在
fn ensure_mp_table(doc: &mut DocumentMut) {
    if doc.get("model_providers").is_none() {
        doc["model_providers"] = Item::Table(Table::new());
    }
}

/// 获取 model_providers 表（只读）
fn mp_table(doc: &DocumentMut) -> Option<&Table> {
    doc.get("model_providers").and_then(|i| i.as_table())
}

/// 删除保留段 model_providers.openai（大小写不敏感）
fn remove_reserved_sections(doc: &mut DocumentMut) {
    ensure_mp_table(doc);
    let keys: Vec<String> = mp_table(doc)
        .map(|t| {
            t.iter()
                .filter(|(k, _)| k.eq_ignore_ascii_case("openai"))
                .map(|(k, _)| k.to_string())
                .collect()
        })
        .unwrap_or_default();
    if let Some(mp) = doc.get_mut("model_providers").and_then(|i| i.as_table_mut()) {
        for k in keys {
            mp.remove(&k);
        }
    }
}

/// 设置/覆盖自定义 provider 段
fn upsert_provider_section(doc: &mut DocumentMut, pid: &str, p: &Provider) {
    ensure_mp_table(doc);
    if let Some(mp) = doc.get_mut("model_providers").and_then(|i| i.as_table_mut()) {
        mp.insert(pid, Item::Table(build_provider_table(pid, p)));
    }
}

/// [auth] 段：官方清 key，自定义写 key
fn update_auth_section(doc: &mut DocumentMut, p: &Provider) {
    if p.ptype == "official" {
        if let Some(table) = doc.get_mut("auth").and_then(|i| i.as_table_mut()) {
            table.remove("api_key");
        }
    } else if !p.api_key.is_empty() {
        if let Some(table) = doc.get_mut("auth").and_then(|i| i.as_table_mut()) {
            table["api_key"] = value_str(&p.api_key);
        }
    }
}

/// 顶层字段：model / review_model / model_provider / model_catalog_json
fn update_top_level(doc: &mut DocumentMut, p: &Provider, pid: &str) {
    let model = if p.model.is_empty() { "gpt-5.5" } else { &p.model };
    doc["model"] = value_str(model);
    doc["review_model"] = value_str(model);
    let provider_id = if p.ptype == "official" {
        "openai" // 内置保留 ID（小写）
    } else {
        pid
    };
    doc["model_provider"] = value_str(provider_id);
    if p.ptype != "official" && !p.catalog_json.is_empty() {
        doc["model_catalog_json"] = value_str(&p.catalog_json);
    } else {
        doc.remove("model_catalog_json");
    }
}

/// 执行完整切换（v4）
pub fn apply_switch(
    config_toml: &Path,
    pid: &str,
    p: &Provider,
    all_providers: &[(String, Provider)],
) -> Result<(), String> {
    let content = std::fs::read_to_string(config_toml)
        .map_err(|e| format!("读取 config.toml 失败: {e}"))?;
    let mut doc = content
        .parse::<DocumentMut>()
        .map_err(|e| format!("解析 config.toml 失败: {e}"))?;

    // 1. 删除保留段（任何大小写的 openai 段）
    remove_reserved_sections(&mut doc);

    if p.ptype == "official" {
        // 官方模式：内置 provider，不写自定义段
        update_auth_section(&mut doc, p);
        update_top_level(&mut doc, p, pid);
    } else {
        // 自定义模式：段名 = provider id
        upsert_provider_section(&mut doc, pid, p);
        // 其他 provider 段保持常驻（并行保存）
        for (other_pid, other_p) in all_providers {
            if other_pid != pid && other_p.ptype != "official" {
                upsert_provider_section(&mut doc, other_pid, other_p);
            }
        }
        update_auth_section(&mut doc, p);
        update_top_level(&mut doc, p, pid);
    }

    std::fs::write(config_toml, doc.to_string())
        .map_err(|e| format!("写入 config.toml 失败: {e}"))?;
    Ok(())
}

/// 读取当前顶层 model_provider
pub fn read_current_provider(config_toml: &Path) -> String {
    let content = std::fs::read_to_string(config_toml).unwrap_or_default();
    let doc = content.parse::<DocumentMut>().unwrap_or_default();
    doc.get("model_provider")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// 读取当前顶层 model
pub fn read_current_model(config_toml: &Path) -> String {
    let content = std::fs::read_to_string(config_toml).unwrap_or_default();
    let doc = content.parse::<DocumentMut>().unwrap_or_default();
    doc.get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

/// 读取当前 provider 段的 base_url（判断官方/自定义模式）
pub fn read_active_base_url(config_toml: &Path) -> String {
    let content = std::fs::read_to_string(config_toml).unwrap_or_default();
    let doc = content.parse::<DocumentMut>().unwrap_or_default();
    let provider = read_current_provider(config_toml);
    if provider.is_empty() || provider.eq_ignore_ascii_case("openai") {
        return String::new();
    }
    mp_table(&doc)
        .and_then(|t| t.get(&provider))
        .and_then(|i| i.as_table())
        .and_then(|t| t.get("base_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// 读取当前顶层 model_catalog_json
pub fn read_current_catalog(config_toml: &Path) -> String {
    let content = std::fs::read_to_string(config_toml).unwrap_or_default();
    let doc = content.parse::<DocumentMut>().unwrap_or_default();
    doc.get("model_catalog_json")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::Provider;
    use std::fs;

    fn temp_config(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cs-v4test-{}-{name}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, content).unwrap();
        path
    }

    fn provider(pid: &str, ptype: &str, base_url: &str, api_key: &str, model: &str, catalog: &str) -> (String, Provider) {
        (
            pid.to_string(),
            Provider {
                display_name: pid.to_string(),
                ptype: ptype.to_string(),
                base_url: base_url.to_string(),
                api_key: api_key.to_string(),
                model: model.to_string(),
                wire_api: "responses".to_string(),
                catalog_json: catalog.to_string(),
                has_keychain_key: false,
            },
        )
    }

    #[test]
    fn switch_to_custom_v4() {
        let path = temp_config("custom", "model_provider = \"OpenAI\"\nmodel = \"gpt-5.5\"\n\n[model_providers.OpenAI]\nname = \"OpenAI\"\nrequires_openai_auth = true\n\n[model_providers.myapi]\nname = \"myapi\"\n\n[auth]\n");
        let myapi = provider("myapi", "custom", "https://myapi.example.com", "sk-test", "gpt-5.6-sol", "/tmp/catalog.json");
        apply_switch(&path, "myapi", &myapi.1, &[myapi.clone()]).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("model_provider = \"myapi\""), "model_provider 未指向段名");
        assert!(content.contains("model = \"gpt-5.6-sol\""), "model 未更新");
        assert!(content.contains("model_catalog_json = \"/tmp/catalog.json\""), "catalog 未写入");
        assert!(!content.contains("model_providers.OpenAI"), "保留段 OpenAI 未删除");
        assert!(!content.contains("model_providers.openai"), "保留段 openai 未删除");
        assert!(content.contains("[model_providers.myapi]"), "自定义段缺失");
        assert!(content.contains("base_url = \"https://myapi.example.com\""), "base_url 未写入");
        assert!(content.contains("experimental_bearer_token = \"sk-test\""), "bearer token 未写入");
        let auth_idx = content.find("[auth]").unwrap();
        assert!(content[auth_idx..].contains("api_key = \"sk-test\""), "[auth] key 未写入");
        fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn switch_to_official_v4() {
        let path = temp_config("official", "model_provider = \"myapi\"\nmodel = \"gpt-5.6-sol\"\nmodel_catalog_json = \"/tmp/catalog.json\"\n\n[model_providers.myapi]\nname = \"myapi\"\nbase_url = \"https://myapi.example.com\"\nexperimental_bearer_token = \"sk-test\"\n\n[model_providers.openai]\nname = \"OpenAI\"\n\n[auth]\napi_key = \"sk-test\"\n");
        let openai = provider("openai", "official", "", "", "gpt-5.5", "");
        apply_switch(&path, "openai", &openai.1, &[openai.clone()]).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("model_provider = \"openai\""), "官方模式 model_provider 应为内置 openai");
        assert!(content.contains("model = \"gpt-5.5\""), "model 未更新");
        assert!(!content.contains("[model_providers.openai]"), "保留段 openai 未删除");
        assert!(!content.contains("[model_providers.OpenAI]"), "保留段 OpenAI 未删除");
        assert!(!content.contains("model_catalog_json"), "官方模式 catalog 未清除");
        let auth_idx = content.find("[auth]").unwrap();
        assert!(!content[auth_idx..].contains("api_key"), "[auth] 残留 key 未清除");
        assert!(content.contains("[model_providers.myapi]"), "自定义常驻段丢失");
        fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn parallel_sections_v4() {
        let path = temp_config("parallel", "model_provider = \"openai\"\nmodel = \"gpt-5.5\"\n\n[auth]\n");
        let myapi = provider("myapi", "custom", "https://myapi.example.com", "sk-test", "gpt-5.6-sol", "");
        let openai = provider("openai", "official", "", "", "gpt-5.5", "");
        apply_switch(&path, "myapi", &myapi.1, &[myapi.clone(), openai.clone()]).unwrap();
        let c1 = fs::read_to_string(&path).unwrap();
        assert!(c1.contains("[model_providers.myapi]"), "自定义段未创建");
        assert!(c1.contains("model_provider = \"myapi\""), "顶层未指向段名");
        apply_switch(&path, "openai", &openai.1, &[myapi.clone(), openai.clone()]).unwrap();
        let c2 = fs::read_to_string(&path).unwrap();
        assert!(c2.contains("[model_providers.myapi]"), "切回官方后常驻段丢失");
        assert!(c2.contains("model_provider = \"openai\""), "官方模式顶层未指向内置 openai");
        fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}
