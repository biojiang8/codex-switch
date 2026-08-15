import { useState } from "react";
import { api } from "../api";
import type { StatusInfo, SwitchConfig } from "../types";

interface Props {
  status: StatusInfo | null;
  config: SwitchConfig | null;
  onChanged: () => void;
  showToast: (text: string, kind: "success" | "error") => void;
}

export default function Home({ status, config, onChanged, showToast }: Props) {
  const [busy, setBusy] = useState<string | null>(null);
  const [steps, setSteps] = useState<string[] | null>(null);
  const [cleaning, setCleaning] = useState(false);

  if (!status || !config) return <div className="empty">加载中…</div>;

  const doSwitch = async (pid: string) => {
    setBusy(pid);
    setSteps(null);
    try {
      const r = await api.switchProvider(pid);
      setSteps(r.steps);
      showToast(r.message, "success");
      onChanged();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setBusy(null);
    }
  };

  const doClean = async (deep: boolean) => {
    setCleaning(true);
    setSteps(null);
    try {
      const r = await api.cleanCache(deep);
      setSteps(r.steps);
      showToast("缓存已清理，会话列表已刷新", "success");
      onChanged();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setCleaning(false);
    }
  };

  const providers = Object.entries(config.providers);

  return (
    <>
      <div className="page-title">首页</div>
      <div className="page-sub">当前状态与一键操作</div>

      <div className="card">
        <h3>当前状态</h3>
        <div className="status-grid">
          <div className="status-item">
            <div className="label">模式</div>
            <div className="value">
              <span className={`pill ${status.current_mode === "官方订阅" ? "official" : "api"}`}>
                {status.current_mode}
              </span>
            </div>
          </div>
          <div className="status-item">
            <div className="label">Provider</div>
            <div className="value">{status.current_display}</div>
          </div>
          <div className="status-item">
            <div className="label">模型</div>
            <div className="value">{status.model}</div>
          </div>
          <div className="status-item">
            <div className="label">认证</div>
            <div className="value">
              <span className={`pill ${status.auth_mode === "chatgpt" ? "official" : "api"}`}>
                {status.auth_mode === "chatgpt" ? "官方账号" : "API Key"}
              </span>
            </div>
          </div>
          <div className="status-item">
            <div className="label">会话数</div>
            <div className="value">{status.thread_count}</div>
          </div>
          <div className="status-item">
            <div className="label">缓存</div>
            <div className="value">{status.cache_size}</div>
          </div>
        </div>
        {status.base_url && (
          <div className="hint mono">Base URL: {status.base_url}</div>
        )}
        {status.catalog_json && (
          <div className="hint mono">模型目录: {status.catalog_json}</div>
        )}
        {!status.codex_installed && (
          <div className="hint" style={{ color: "var(--red)" }}>
            ⚠ 未检测到 ~/.codex 目录：请先安装 ChatGPT 桌面应用
          </div>
        )}
      </div>

      <div className="card">
        <h3>一键切换</h3>
        {providers.length === 0 ? (
          <div className="empty">还没有 Provider，去「Provider 管理」添加</div>
        ) : (
          providers.map(([pid, p]) => {
            const isCurrent = pid === status.current_provider;
            return (
              <div key={pid} className={`provider-row ${isCurrent ? "current" : ""}`}>
                <div className="provider-info">
                  <div className="provider-name">
                    {p.display_name}
                    <span className={`pill ${p.type === "official" ? "official" : "api"}`}>
                      {p.type === "official" ? "官方" : "API"}
                    </span>
                    {isCurrent && <span className="pill dim">当前</span>}
                  </div>
                  <div className="provider-meta">
                    模型: {p.model || "未设置"}
                    {p.base_url && ` · ${p.base_url}`}
                  </div>
                </div>
                <button
                  className="btn primary small"
                  disabled={busy !== null || isCurrent}
                  onClick={() => doSwitch(pid)}
                >
                  {busy === pid ? "切换中…" : isCurrent ? "已启用" : "切换"}
                </button>
              </div>
            );
          })
        )}
      </div>

      <div className="card">
        <h3>清缓存刷新</h3>
        <div className="desc" style={{ fontSize: 12.5, color: "var(--text-dim)", marginBottom: 10 }}>
          会话列表异常、模型列表不刷新、切换后界面错乱时使用。只清理可再生缓存，绝不删对话记录。
        </div>
        <div style={{ display: "flex", gap: 10 }}>
          <button className="btn green" disabled={cleaning} onClick={() => doClean(false)}>
            {cleaning ? "清理中…" : "清缓存刷新"}
          </button>
          <button className="btn" disabled={cleaning} onClick={() => doClean(true)}>
            {cleaning ? "清理中…" : "深度清理（--deep）"}
          </button>
        </div>
      </div>

      {steps && (
        <div className="card">
          <h3>执行日志</h3>
          <div className="steps-log">
            {steps.map((s, i) => (
              <div key={i} className="info">
                {s}
              </div>
            ))}
          </div>
        </div>
      )}
    </>
  );
}
