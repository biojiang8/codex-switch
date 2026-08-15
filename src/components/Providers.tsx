import { useState } from "react";
import { api } from "../api";
import type { Provider, SwitchConfig } from "../types";

interface Props {
  config: SwitchConfig | null;
  onChanged: () => void;
  showToast: (text: string, kind: "success" | "error") => void;
}

const emptyForm: Provider = {
  display_name: "",
  type: "custom",
  base_url: "",
  api_key: "",
  model: "",
  wire_api: "responses",
  catalog_json: "",
  has_keychain_key: false,
};

export default function Providers({ config, onChanged, showToast }: Props) {
  const [editing, setEditing] = useState<string | null>(null); // null=不编辑, "new"=新增, 其他=编辑 id
  const [form, setForm] = useState<Provider>({ ...emptyForm });
  const [formPid, setFormPid] = useState("");
  const [busy, setBusy] = useState(false);
  const [testState, setTestState] = useState<"idle" | "testing" | "ok" | "fail">("idle");
  const [testMsg, setTestMsg] = useState("");

  if (!config) return <div className="empty">加载中…</div>;

  const startNew = () => {
    setForm({ ...emptyForm });
    setFormPid("");
    setEditing("new");
    setTestState("idle");
  };

  const startEdit = (pid: string, p: Provider) => {
    setFormPid(pid);
    setForm({
      ...p,
      api_key: "", // 不显示已有 key，留空 = 保留 Keychain 中的
    });
    setEditing(pid);
    setTestState("idle");
  };

  const save = async () => {
    if (!formPid.trim() || !form.display_name.trim()) {
      showToast("请填写 Provider ID 和显示名称", "error");
      return;
    }
    if (form.type === "custom" && !form.base_url.trim()) {
      showToast("自定义 Provider 需要 Base URL", "error");
      return;
    }
    setBusy(true);
    try {
      await api.saveProvider(formPid.trim(), form);
      showToast(`已保存 ${form.display_name}`, "success");
      setEditing(null);
      onChanged();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setBusy(false);
    }
  };

  const remove = async (pid: string, name: string) => {
    if (!confirm(`确定删除 Provider「${name}」？其钥匙串中的 Key 也会一并删除。`)) return;
    try {
      await api.deleteProvider(pid);
      showToast(`已删除 ${name}`, "success");
      onChanged();
    } catch (e) {
      showToast(String(e), "error");
    }
  };

  const test = async () => {
    if (!form.base_url.trim() || !form.api_key.trim()) {
      setTestState("fail");
      setTestMsg("测试需要 Base URL 和 API Key");
      return;
    }
    setTestState("testing");
    try {
      const r = await api.testConnection(form.base_url.trim(), form.api_key.trim());
      if (r.ok) {
        setTestState("ok");
        setTestMsg(r.model_count > 0 ? `连接成功，发现 ${r.model_count} 个模型` : "连接成功");
      }
    } catch (e) {
      setTestState("fail");
      setTestMsg(String(e));
    }
  };

  return (
    <>
      <div className="page-title">Provider 管理</div>
      <div className="page-sub">
        添加任意 OpenAI 兼容服务商；API Key 存入 macOS 钥匙串，配置文件中不留明文
      </div>

      <div className="card">
        <h3>已配置的 Provider（{Object.keys(config.providers).length}）</h3>
        {Object.entries(config.providers).length === 0 && (
          <div className="empty">还没有 Provider，点右上角添加</div>
        )}
        {Object.entries(config.providers).map(([pid, p]) => (
          <div key={pid} className="provider-row">
            <div className="provider-info">
              <div className="provider-name">
                {p.display_name}
                <span className={`pill ${p.type === "official" ? "official" : "api"}`}>
                  {p.type === "official" ? "官方" : "API"}
                </span>
                {p.type === "custom" &&
                  (p.has_keychain_key ? (
                    <span className="key-badge">钥匙串 ✓</span>
                  ) : (
                    <span className="no-key-badge">无 Key</span>
                  ))}
              </div>
              <div className="provider-meta">
                ID: {pid} · 模型: {p.model || "未设置"}
                {p.base_url && ` · ${p.base_url}`}
                {p.catalog_json && ` · 目录: ${p.catalog_json}`}
              </div>
            </div>
            <button className="btn small" onClick={() => startEdit(pid, p)}>
              编辑
            </button>
            <button className="btn danger small" onClick={() => remove(pid, p.display_name)}>
              删除
            </button>
          </div>
        ))}
        <div style={{ marginTop: 12 }}>
          <button className="btn primary" onClick={startNew}>
            + 添加 Provider
          </button>
        </div>
      </div>

      {editing !== null && (
        <div className="card">
          <h3>{editing === "new" ? "添加 Provider" : `编辑 Provider（${editing}）`}</h3>
          <div className="form-grid">
            <div className="form-field">
              <label>Provider ID</label>
              <input
                value={formPid}
                disabled={editing !== "new"}
                placeholder="如 deepseek"
                onChange={(e) => setFormPid(e.target.value)}
              />
            </div>
            <div className="form-field">
              <label>显示名称</label>
              <input
                value={form.display_name}
                placeholder="如 DeepSeek"
                onChange={(e) => setForm({ ...form, display_name: e.target.value })}
              />
            </div>
            <div className="form-field">
              <label>类型</label>
              <select
                value={form.type}
                onChange={(e) =>
                  setForm({ ...form, type: e.target.value as "official" | "custom" })
                }
              >
                <option value="custom">自定义 API</option>
                <option value="official">OpenAI 官方</option>
              </select>
            </div>
            <div className="form-field">
              <label>接口类型（wire_api）</label>
              <select
                value={form.wire_api}
                onChange={(e) =>
                  setForm({ ...form, wire_api: e.target.value as "responses" | "chat" })
                }
              >
                <option value="responses">responses（原生）</option>
                <option value="chat">chat（/chat/completions）</option>
              </select>
            </div>
            {form.type === "custom" && (
              <>
                <div className="form-field full">
                  <label>Base URL</label>
                  <input
                    value={form.base_url}
                    placeholder="https://api.example.com"
                    onChange={(e) => setForm({ ...form, base_url: e.target.value })}
                  />
                </div>
                <div className="form-field">
                  <label>
                    API Key{" "}
                    {form.has_keychain_key ? "（钥匙串已有，留空保留）" : "（存入钥匙串）"}
                  </label>
                  <input
                    type="password"
                    value={form.api_key}
                    placeholder="sk-..."
                    onChange={(e) => setForm({ ...form, api_key: e.target.value })}
                  />
                </div>
                <div className="form-field" style={{ justifyContent: "flex-end" }}>
                  <div style={{ marginTop: 22 }}>
                    <button
                      className="btn small"
                      disabled={testState === "testing"}
                      onClick={test}
                    >
                      {testState === "testing" ? "测试中…" : "测试连接"}
                    </button>
                    {testState !== "idle" && (
                      <div
                        className="hint"
                        style={{ color: testState === "ok" ? "var(--green)" : "var(--red)" }}
                      >
                        {testState === "testing" ? "测试中…" : testMsg}
                      </div>
                    )}
                  </div>
                </div>
              </>
            )}
            <div className="form-field full">
              <label>模型 ID</label>
              <input
                value={form.model}
                placeholder="如 deepseek-chat / gpt-5.5"
                onChange={(e) => setForm({ ...form, model: e.target.value })}
              />
            </div>
            <div className="form-field full">
              <label>模型目录（catalog_json，可选）</label>
              <input
                value={form.catalog_json}
                placeholder="/Users/xxx/.codex/model-catalogs/xxx.json（留空自动移除）"
                onChange={(e) => setForm({ ...form, catalog_json: e.target.value })}
              />
            </div>
          </div>
          <div style={{ display: "flex", gap: 10, marginTop: 16 }}>
            <button className="btn primary" disabled={busy} onClick={save}>
              {busy ? "保存中…" : "保存"}
            </button>
            <button className="btn ghost" onClick={() => setEditing(null)}>
              取消
            </button>
          </div>
        </div>
      )}
    </>
  );
}
