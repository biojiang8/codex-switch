//! config.toml 编辑核心（移植 CLI v3.0 的 Python 逻辑）
//! 使用 toml_edit 保留注释与格式，只做最小修改

use crate::codex::Provider;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table, Value};

/// 写入 Provider 段（并行常驻，不覆盖已有段）
/// 注意：[model_providers.XXX] 是嵌套表，必须用 doc["model_providers"]["XXX"] 访问
fn ensure_provider_section(doc: &mut DocumentMut, pid: &str, p: &Provider) {
    let exists = doc
        .get("model_providers")
        .and_then(|i| i.as_table())
        .map(|t| t.contains_key(pid))
        .unwrap_or(false);
    if exists {
        return;
    }
    let mut table = Table::new();
    let name = if pid.eq_ignore_ascii_case("openai") {
        "OpenAI".to_string()
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
            table["api_key"] = value_str(&p.api_key);
        }
    }
    // 确保 model_providers 表存在
    if doc.get("model_providers").is_none() {
        doc["model_providers"] = Item::Table(Table::new());
    }
    if let Some(mp) = doc.get_mut("model_providers").and_then(|i| i.as_table_mut()) {
        mp.insert(pid, Item::Table(table));
    }
}

fn value_str(s: &str) -> Item {
    Item::Value(Value::from(s.to_string()))
}
fn value_bool(b: bool) -> Item {
    Item::Value(Value::from(b))
}

/// 把目标 provider 镜像进活跃段 [model_providers.OpenAI]
/// （清掉 base_url/api_key/认证字段后重写，保留其他未知字段）
fn mirror_into_active_section(doc: &mut DocumentMut, p: &Provider) {
    // 活跃段：model_providers.OpenAI（嵌套表）
    let mut table = doc
        .get("model_providers")
        .and_then(|i| i.as_table())
        .and_then(|t| t.get("OpenAI"))
        .and_then(|i| i.as_table())
        .cloned()
        .unwrap_or_default();

    // 删除需要重写的字段
    for key in [
        "base_url",
        "api_key",
        "requires_openai_auth",
        "experimental_bearer_token",
        "env_key",
        "env_key_instructions",
    ] {
        table.remove(key);
    }

    table["name"] = value_str("OpenAI");
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
            table["api_key"] = value_str(&p.api_key);
        }
    }

    if doc.get("model_providers").is_none() {
        doc["model_providers"] = Item::Table(Table::new());
    }
    if let Some(mp) = doc.get_mut("model_providers").and_then(|i| i.as_table_mut()) {
        mp.insert("OpenAI", Item::Table(table));
    }
}

/// 顶层字段管理：model / review_model / model_provider / model_catalog_json
fn update_top_level(doc: &mut DocumentMut, p: &Provider) {
    doc["model"] = value_str(if p.model.is_empty() { "gpt-5.5" } else { &p.model });
    doc["review_model"] = value_str(if p.model.is_empty() { "gpt-5.5" } else { &p.model });
    doc["model_provider"] = value_str("OpenAI");
    if !p.catalog_json.is_empty() {
        doc["model_catalog_json"] = value_str(&p.catalog_json);
    } else {
        doc.remove("model_catalog_json");
    }
}

/// [auth] 段：官方模式移除残留 api_key，custom 模式写入
fn update_auth_section(doc: &mut DocumentMut, p: &Provider) {
    let auth_key = "auth";
    if p.ptype == "official" {
        if let Some(table) = doc.get_mut(auth_key).and_then(|i| i.as_table_mut()) {
            table.remove("api_key");
        }
    } else if !p.api_key.is_empty() {
        if let Some(table) = doc.get_mut(auth_key).and_then(|i| i.as_table_mut()) {
            table["api_key"] = value_str(&p.api_key);
        }
    }
}

/// 执行完整切换：返回 (是否修改了文件, 新 model)
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

    // 1. 确保所有 provider 常驻段存在（并行保存）
    for (other_pid, other_p) in all_providers {
        ensure_provider_section(&mut doc, other_pid, other_p);
    }

    // 2. 镜像目标 provider 进活跃段
    mirror_into_active_section(&mut doc, p);

    // 3. 顶层字段
    update_top_level(&mut doc, p);

    // 4. [auth] 段
    update_auth_section(&mut doc, p);

    std::fs::write(config_toml, doc.to_string())
        .map_err(|e| format!("写入 config.toml 失败: {e}"))?;
    let _ = pid;
    Ok(())
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

/// 读取当前活跃段 base_url（判断官方/自定义模式）
pub fn read_active_base_url(config_toml: &Path) -> String {
    let content = std::fs::read_to_string(config_toml).unwrap_or_default();
    let doc = content.parse::<DocumentMut>().unwrap_or_default();
    doc.get("model_providers")
        .and_then(|i| i.as_table())
        .and_then(|t| t.get("OpenAI"))
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
        let dir = std::env::temp_dir().join(format!("cs-test-{}-{name}", std::process::id()));
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
    fn switch_to_custom_mirrors_active_section() {
        let path = temp_config("custom",
            "model_provider = \"OpenAI\"\nmodel = \"gpt-5.5\"\n\n[model_providers.OpenAI]\nname = \"OpenAI\"\nrequires_openai_auth = true\n\n[model_providers.myapi]\nname = \"myapi\"\n\n[auth]\n",
        );
        let myapi = provider("myapi", "custom", "https://myapi.com", "sk-test", "gpt-5.6-sol", "/tmp/catalog.json");
        apply_switch(&path, "myapi", &myapi.1, &[myapi.clone()]).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        eprintln!("=== 切换后内容 ===\n{content}");
        assert!(content.contains("model = \"gpt-5.6-sol\""), "model 未更新");
        assert!(content.contains("model_provider = \"OpenAI\""), "model_provider 被改");
        assert!(content.contains("model_catalog_json = \"/tmp/catalog.json\""), "catalog 未写入");
        assert!(content.contains("base_url = \"https://myapi.com\""), "base_url 未写入活跃段");
        assert!(content.contains("requires_openai_auth = false"), "认证标记未更新");
        assert!(content.contains("api_key = \"sk-test\""), "api_key 未写入活跃段");
        // [auth] 段也应有 key
        let auth_idx = content.find("[auth]").unwrap();
        assert!(content[auth_idx..].contains("api_key = \"sk-test\""), "[auth] 段 key 未写入");
        // 并行段保留
        assert!(content.contains("[model_providers.myapi]"), "并行段丢失");
        fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn switch_to_official_cleans_residue() {
        let path = temp_config("official",
            "model_provider = \"OpenAI\"\nmodel = \"gpt-5.6-sol\"\nmodel_catalog_json = \"/tmp/myapi-catalog.json\"\n\n[model_providers.OpenAI]\nname = \"OpenAI\"\nbase_url = \"https://myapi.com\"\nrequires_openai_auth = false\napi_key = \"sk-test\"\n\n[auth]\napi_key = \"sk-test\"\n",
        );
        let openai = provider("openai", "official", "", "", "gpt-5.5", "");
        apply_switch(&path, "openai", &openai.1, &[openai.clone()]).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        eprintln!("=== official 切换后 ===\n{content}");
        assert!(content.contains("model = \"gpt-5.5\""), "model 未更新");
        assert!(!content.contains("model_catalog_json"), "残留 catalog 未清除");
        assert!(!content.contains("base_url"), "残留 base_url 未清除");
        assert!(content.contains("requires_openai_auth = true"), "官方认证标记未写入");
        let auth_idx = content.find("[auth]").unwrap();
        assert!(!content[auth_idx..].contains("api_key"), "[auth] 段残留 key 未清除");
        fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn parallel_sections_preserved() {
        let path = temp_config("parallel",
            "model_provider = \"OpenAI\"\nmodel = \"gpt-5.5\"\n\n[model_providers.OpenAI]\nname = \"OpenAI\"\nrequires_openai_auth = true\n\n[auth]\n",
        );
        let myapi = provider("myapi", "custom", "https://myapi.com", "sk-test", "gpt-5.6-sol", "");
        let openai = provider("openai", "official", "", "", "gpt-5.5", "");
        // 切 myapi（无 myapi 段 → 自动创建）
        apply_switch(&path, "myapi", &myapi.1, &[myapi.clone(), openai.clone()]).unwrap();
        let c1 = fs::read_to_string(&path).unwrap();
        assert!(c1.contains("[model_providers.myapi]"), "常驻段未创建");
        // 切回 openai → myapi 段仍在
        apply_switch(&path, "openai", &openai.1, &[myapi.clone(), openai.clone()]).unwrap();
        let c2 = fs::read_to_string(&path).unwrap();
        assert!(c2.contains("[model_providers.myapi]"), "切回后常驻段丢失");
        fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}
