import { useState } from "react";
import { api } from "../api";
import type { SwitchConfig } from "../types";

interface Props {
  config: SwitchConfig | null;
  onChanged: () => void;
  showToast: (text: string, kind: "success" | "error") => void;
}

export default function Settings({ config, onChanged, showToast }: Props) {
  const [defaultProvider, setDefaultProvider] = useState(config?.default_provider ?? "");
  const [zshrcEnv, setZshrcEnv] = useState(config?.zshrc_env ?? true);
  const [busy, setBusy] = useState(false);
  const [keychainOk, setKeychainOk] = useState<boolean | null>(null);

  if (!config) return <div className="empty">加载中…</div>;

  const save = async () => {
    setBusy(true);
    try {
      await api.saveGeneralSettings(defaultProvider, zshrcEnv);
      showToast("设置已保存", "success");
      onChanged();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const checkKeychain = async () => {
    const s = await api.keychainAvailable();
    setKeychainOk(s.available);
  };

  return (
    <>
      <div className="page-title">设置</div>
      <div className="page-sub">通用偏好与安全状态</div>

      <div className="card">
        <h3>通用设置</h3>
        <div className="form-grid">
          <div className="form-field">
            <label>默认 Provider</label>
            <select value={defaultProvider} onChange={(e) => setDefaultProvider(e.target.value)}>
              {Object.entries(config.providers).map(([pid, p]) => (
                <option key={pid} value={pid}>
                  {p.display_name}（{pid}）
                </option>
              ))}
            </select>
          </div>
          <div className="form-field" style={{ justifyContent: "flex-end" }}>
            <label className="checkbox-row" style={{ marginTop: 20 }}>
              <input
                type="checkbox"
                checked={zshrcEnv}
                onChange={(e) => setZshrcEnv(e.target.checked)}
              />
              切换时同步 ~/.zshrc 环境变量（终端用 Codex CLI 时建议开启）
            </label>
          </div>
        </div>
        <div style={{ marginTop: 16 }}>
          <button className="btn primary" disabled={busy} onClick={save}>
            {busy ? "保存中…" : "保存设置"}
          </button>
        </div>
      </div>

      <div className="card">
        <h3>安全</h3>
        <div className="hint">
          API Key 存储在 macOS 钥匙串（Keychain）中，配置文件 ~/.codex/codex-switch-config.json
          只保存非敏感信息，可以放心把配置示例推送到 GitHub。
        </div>
        <div style={{ marginTop: 10 }}>
          <button className="btn small" onClick={checkKeychain}>
            检查钥匙串状态
          </button>
          {keychainOk !== null && (
            <span
              className="hint"
              style={{
                marginLeft: 10,
                color: keychainOk ? "var(--green)" : "var(--red)",
              }}
            >
              {keychainOk ? "✓ 钥匙串可用" : "✘ 钥匙串不可用（请检查系统安全设置）"}
            </span>
          )}
        </div>
      </div>

      <div className="card">
        <h3>关于</h3>
        <div className="hint">
          Codex Switch 桌面版 v3.0（Tauri）
          <br />
          功能：ChatGPT 官方账户 / 各种 API 调用 / 清缓存刷新
          <br />
          切换原理：config.toml 并行段架构，只镜像活跃段 [model_providers.OpenAI]，
          绝不修改 threads 表 model_provider，对话记录始终保留。
        </div>
      </div>
    </>
  );
}
