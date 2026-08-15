//! Codex Switch 桌面版 — Tauri 后端入口
//! 模块：
//!   - codex:        配置模型与读写
//!   - config_toml:  config.toml 编辑核心
//!   - keychain:     macOS Keychain 存取
//!   - switch:       切换 Provider / 保存删除 Provider / 测试连接
//!   - clean:        清缓存刷新
//!   - status:       状态读取

mod clean;
mod codex;
mod config_toml;
mod keychain;
mod status;
mod switch;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            switch::switch_provider,
            switch::get_config,
            switch::save_provider,
            switch::delete_provider,
            switch::save_general_settings,
            switch::test_connection,
            clean::clean_cache,
            status::get_status,
            keychain::is_available,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
