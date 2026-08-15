//! 清缓存刷新（移植 CLI v3.0 clean 命令）
//! 安全原则：
//!   - 只清理"可再生"缓存：~/.codex/cache/、临时目录、session_index.jsonl（可从 SQLite 重建）
//!   - 绝不删除：state_5.sqlite（对话本体）、archived_sessions、rollout、config/auth/env
//!   - --deep 重置全局状态缓存键前强制备份

use crate::codex::{
    cache_dir_path, global_state_path, session_index_path, state_db_path, backups_dir,
};
use crate::switch::restart_chatgpt;
use rusqlite::Connection;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct CleanResult {
    pub ok: bool,
    pub steps: Vec<String>,
}

fn step(steps: &mut Vec<String>, msg: String) {
    steps.push(msg);
}

/// 目录大小（人类可读）
pub fn dir_size_human(path: &Path) -> String {
    let mut total: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Ok(meta) = std::fs::symlink_metadata(&p) {
                if meta.file_type().is_symlink() {
                    continue;
                }
                if meta.is_dir() {
                    total += dir_size_bytes(&p);
                } else {
                    total += meta.len();
                }
            }
        }
    }
    format_size(total)
}

fn dir_size_bytes(path: &Path) -> u64 {
    let mut total: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Ok(meta) = std::fs::symlink_metadata(&p) {
                if meta.is_dir() {
                    total += dir_size_bytes(&p);
                } else {
                    total += meta.len();
                }
            }
        }
    }
    total
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// 清空 ~/.codex/cache/ 下的所有子项（保留目录本身）
fn clean_cache_dir(steps: &mut Vec<String>) {
    let cache = cache_dir_path();
    if !cache.exists() {
        step(steps, "缓存目录不存在，跳过".into());
        return;
    }
    let before = dir_size_human(&cache);
    let mut n = 0u32;
    if let Ok(entries) = std::fs::read_dir(&cache) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let _ = std::fs::remove_dir_all(&p);
            } else {
                let _ = std::fs::remove_file(&p);
            }
            n += 1;
        }
    }
    let after = dir_size_human(&cache);
    step(
        steps,
        format!("已清理 cache/ 下 {n} 项（{before} → {after}，Codex 会自动重建）"),
    );
}

/// 重建 session_index.jsonl（从 state_5.sqlite 重建，不丢对话）
fn rebuild_session_index(steps: &mut Vec<String>) {
    let db_path = state_db_path();
    let index_path = session_index_path();
    if !db_path.exists() {
        step(steps, "state_5.sqlite 不存在，跳过索引重建".into());
        return;
    }
    match Connection::open(&db_path) {
        Ok(conn) => {
            let result = conn.prepare(
                "SELECT id, title, updated_at_ms FROM threads WHERE title IS NOT NULL AND title != '' ORDER BY updated_at_ms DESC",
            );
            match result {
                Ok(mut stmt) => {
                    let rows: Vec<(String, String, Option<i64>)> = stmt
                        .query_map([], |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get::<_, Option<i64>>(2)?,
                            ))
                        })
                        .map(|it| it.flatten().collect())
                        .unwrap_or_default();
                    let mut out = String::new();
                    for (id, title, updated_ms) in &rows {
                        let updated_at = updated_ms.map(|ms| {
                            let secs = ms / 1000;
                            let nanos = (ms % 1000) as u32 * 1_000_000;
                            let dt = std::time::UNIX_EPOCH + std::time::Duration::new(secs as u64, nanos);
                            let datetime = chrono_iso(&dt);
                            datetime
                        }).unwrap_or_default();
                        let entry = serde_json::json!({
                            "id": id,
                            "thread_name": title,
                            "updated_at": updated_at,
                        });
                        out.push_str(&entry.to_string());
                        out.push('\n');
                    }
                    // 备份旧索引
                    if index_path.exists() {
                        let _ = std::fs::copy(&index_path, format!("{}.bak", index_path.display()));
                    }
                    if std::fs::write(&index_path, out).is_ok() {
                        step(steps, format!("已重建 {} 条会话索引", rows.len()));
                    } else {
                        step(steps, "写入 session_index.jsonl 失败".into());
                    }
                }
                Err(e) => step(steps, format!("查询 threads 失败: {e}")),
            }
        }
        Err(e) => step(steps, format!("打开 state_5.sqlite 失败: {e}")),
    }
}

/// 简易 ISO8601 时间格式化（UTC）
fn chrono_iso(dt: &std::time::SystemTime) -> String {
    let secs = dt
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // 1970-01-01 起的天数 → 年/月/日（简化：足够用于排序展示）
    let y = 1970 + days / 365;
    let doy = days % 365;
    let (mo, d) = if doy < 31 { (1, doy + 1) } else if doy < 59 { (2, doy - 30) } else if doy < 90 { (3, doy - 58) } else if doy < 120 { (4, doy - 89) } else if doy < 151 { (5, doy - 119) } else if doy < 181 { (6, doy - 150) } else if doy < 212 { (7, doy - 180) } else if doy < 243 { (8, doy - 211) } else if doy < 273 { (9, doy - 242) } else if doy < 304 { (10, doy - 272) } else if doy < 334 { (11, doy - 303) } else { (12, doy - 333) };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}.000000Z")
}

/// 清理临时目录（--deep）
fn clean_tmp_dirs(steps: &mut Vec<String>) {
    for dir_name in [".tmp", "tmp"] {
        let dir = crate::codex::codex_home().join(dir_name);
        if dir.exists() {
            let size = dir_size_human(&dir);
            let _ = std::fs::remove_dir_all(&dir);
            let _ = std::fs::create_dir_all(&dir);
            step(steps, format!("已清理临时目录 {dir_name}/（{size}）"));
        }
    }
}

/// 重置全局状态缓存键（--deep，先备份）
fn reset_global_state(steps: &mut Vec<String>) {
    let gs = global_state_path();
    if !gs.exists() {
        return;
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();
    let bak = backups_dir().join(format!("{}.{ts}.bak", gs.file_name().unwrap_or_default().to_string_lossy()));
    if std::fs::copy(&gs, &bak).is_ok() {
        step(steps, format!("全局状态已备份: {}", bak.display()));
    }
    // 只移除可再生的键，保留认证与迁移标记
    if let Ok(content) = std::fs::read_to_string(&gs) {
        if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(obj) = json.as_object_mut() {
                for key in [
                    "electron-persisted-atom-state",
                    "electron-main-window-bounds",
                    "queued-follow-ups",
                    "project-order",
                    "electron-avatar-overlay-bounds",
                ] {
                    obj.remove(key);
                }
            }
            let _ = std::fs::write(&gs, serde_json::to_string_pretty(&json).unwrap_or_default());
            step(steps, "全局状态中的窗口/会话缓存键已重置（认证与迁移标记保留）".into());
        }
    }
}

/// 主清缓存刷新命令
#[tauri::command]
pub fn clean_cache(deep: Option<bool>, skip_restart: Option<bool>) -> Result<CleanResult, String> {
    let mut steps: Vec<String> = Vec::new();

    if deep.unwrap_or(false) {
        reset_global_state(&mut steps);
        clean_tmp_dirs(&mut steps);
    }

    clean_cache_dir(&mut steps);
    rebuild_session_index(&mut steps);
    restart_chatgpt(skip_restart.unwrap_or(false), &mut steps);

    Ok(CleanResult { ok: true, steps })
}
