//! 跨平台密钥存储
//! 每个 provider 一个独立条目（service = "codex-switch", account = provider_id）
//!   macOS   → Apple Keychain（钥匙串）
//!   Windows → Windows Credential Manager（凭据管理器）
//!   Linux   → Secret Service（gnome-keyring / libsecret）
//! 配置文件（codex-switch-config.json）中永不保存明文 Key

use keyring::Entry;
use serde::Serialize;

pub const KEYCHAIN_SERVICE: &str = "codex-switch";

#[derive(Debug, Serialize)]
pub struct KeychainStatus {
    pub available: bool,
    pub message: String,
}

fn entry(provider_id: &str) -> Entry {
    Entry::new(KEYCHAIN_SERVICE, provider_id).expect("创建密钥条目失败")
}

/// 读取某 provider 的 key
pub fn get_key(provider_id: &str) -> String {
    match entry(provider_id).get_password() {
        Ok(key) => key,
        Err(_) => String::new(),
    }
}

/// 保存某 provider 的 key（空值 = 删除）
pub fn set_key(provider_id: &str, api_key: &str) -> Result<(), String> {
    if api_key.is_empty() {
        return delete_key(provider_id);
    }
    entry(provider_id)
        .set_password(api_key)
        .map_err(|e| format!("保存密钥失败: {e}"))
}

/// 删除某 provider 的 key
pub fn delete_key(provider_id: &str) -> Result<(), String> {
    match entry(provider_id).delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("删除密钥失败: {e}")),
    }
}

/// 检查密钥存储是否可用
#[tauri::command]
pub fn is_available() -> KeychainStatus {
    // 尝试写入一个临时条目来验证可用性
    let probe = Entry::new(KEYCHAIN_SERVICE, "__probe__");
    match probe {
        Ok(e) => match e.set_password("probe") {
            Ok(()) => {
                let _ = e.delete_credential();
                KeychainStatus {
                    available: true,
                    message: "密钥存储可用".into(),
                }
            }
            Err(err) => KeychainStatus {
                available: false,
                message: format!("密钥存储不可用: {err}"),
            },
        },
        Err(err) => KeychainStatus {
            available: false,
            message: format!("密钥存储不可用: {err}"),
        },
    }
}
