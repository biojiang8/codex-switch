import { useState } from "react";
import { api } from "../api";
import type { Provider } from "../types";

interface Props {
  onDone: () => void;
}

export default function SetupWizard({ onDone }: Props) {
  const [step, setStep] = useState(1);
  const [mode, setMode] = useState<"official" | "custom" | null>(null);
  const [busy, setBusy] = useState(false);

  // API 表单
  const [pid, setPid] = useState("");
  const [name, setName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [wireApi, setWireApi] = useState<"responses" | "chat">("responses");
  const [testState, setTestState] = useState<"idle" | "testing" | "ok" | "fail">("idle");
  const [testMsg, setTestMsg] = useState("");

  const nextFromMode = () => {
    if (mode === "official") {
      finishOfficial();
    } else {
      setStep(3);
    }
  };

  const finishOfficial = async () => {
    setBusy(true);
    try {
      const provider: Provider = {
        display_name: "OpenAI 官方",
        type: "official",
        base_url: "",
        api_key: "",
        model: "gpt-5.5",
        wire_api: "responses",
        catalog_json: "",
        has_keychain_key: false,
      };
      await api.saveProvider("openai", provider);
      await api.switchProvider("openai", true);
      onDone();
    } catch (e) {
      alert(String(e));
    } finally {
      setBusy(false);
    }
  };

  const testConnection = async () => {
    if (!baseUrl.trim() || !apiKey.trim()) {
      setTestState("fail");
      setTestMsg("请先填写 Base URL 和 API Key");
      return;
    }
    setTestState("testing");
    try {
      const r = await api.testConnection(baseUrl.trim(), apiKey.trim());
      if (r.ok) {
        setTestState("ok");
        setTestMsg(
          r.model_count > 0 ? `连接成功，发现 ${r.model_count} 个模型` : "连接成功"
        );
      }
    } catch (e) {
      setTestState("fail");
      setTestMsg(String(e));
    }
  };

  const finishCustom = async () => {
    if (!pid.trim() || !name.trim() || !baseUrl.trim() || !model.trim()) {
      alert("请填写 Provider ID、名称、Base URL 和模型");
      return;
    }
    setBusy(true);
    try {
      const provider: Provider = {
        display_name: name.trim(),
        type: "custom",
        base_url: baseUrl.trim(),
        api_key: apiKey.trim(), // 后端会存入 Keychain，配置内不留明文
        model: model.trim(),
        wire_api: wireApi,
        catalog_json: "",
        has_keychain_key: !!apiKey.trim(),
      };
      await api.saveProvider(pid.trim(), provider);
      await api.switchProvider(pid.trim(), true);
      onDone();
    } catch (e) {
      alert(String(e));
    } finally {
      setBusy(false);
    }
  };

  const stepIndicator = (n: number, label: string) => (
    <div className={`wizard-step ${step === n ? "active" : ""} ${step > n ? "done" : ""}`}>
      <span className="num">{step > n ? "✓" : n}</span> {label}
    </div>
  );

  return (
    <div className="wizard">
      <div className="wizard-steps">
        {stepIndicator(1, "模式选择")}
        {stepIndicator(2, "官方登录")}
        {stepIndicator(3, "API 配置")}
      </div>

      {step === 1 && (
        <div className="wizard-box">
          <h2>欢迎使用 Codex Switch 👋</h2>
          <div className="desc">
            一个工具管三件事：ChatGPT 官方账户、各种 API 调用、清缓存刷新。
            选择你的使用方式开始配置。
          </div>
          <div className="mode-cards">
            <div
              className={`mode-card ${mode === "official" ? "selected" : ""}`}
              onClick={() => setMode("official")}
            >
              <div className="title">ChatGPT 官方账户</div>
              <div className="sub">
                使用你的 ChatGPT 登录账号（Plus/Pro 订阅），
                无需任何 API Key。适合已有官方订阅的用户。
              </div>
            </div>
            <div
              className={`mode-card ${mode === "custom" ? "selected" : ""}`}
              onClick={() => setMode("custom")}
            >
              <div className="title">API 调用（各种服务商）</div>
              <div className="sub">
                任意 OpenAI 兼容接口：官方 API、第三方中转、DeepSeek、Kimi 等。
                填入 Base URL + API Key 即可，Key 存进 macOS 钥匙串。
              </div>
            </div>
          </div>
          <div className="wizard-actions">
            <span />
            <button className="btn primary" disabled={!mode || busy} onClick={nextFromMode}>
              {mode === "official" ? "使用官方账户" : "下一步"}
            </button>
          </div>
        </div>
      )}

      {step === 2 && (
        <div className="wizard-box">
          <h2>官方账户模式</h2>
          <div className="desc">
            将使用 ChatGPT 桌面应用登录的账号。切换时会：
            <br />• 清除 config.toml / .env / .zshrc 中的 API Key
            <br />• 认证切回 chatgpt 模式
            <br />• 自动重启 ChatGPT 应用生效
          </div>
          <div className="wizard-actions">
            <button className="btn ghost" onClick={() => setStep(1)}>
              上一步
            </button>
            <button className="btn primary" disabled={busy} onClick={finishOfficial}>
              {busy ? "配置中…" : "确认使用官方账户"}
            </button>
          </div>
        </div>
      )}

      {step === 3 && (
        <div className="wizard-box">
          <h2>配置 API Provider</h2>
          <div className="desc">
            API Key 会存入 macOS 钥匙串（Keychain），配置文件里不留明文，可放心分享配置。
          </div>
          <div className="form-grid">
            <div className="form-field">
              <label>Provider ID（唯一标识）</label>
              <input
                placeholder="如 deepseek、gpt-api"
                value={pid}
                onChange={(e) => setPid(e.target.value)}
              />
            </div>
            <div className="form-field">
              <label>显示名称</label>
              <input
                placeholder="如 DeepSeek"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </div>
            <div className="form-field full">
              <label>Base URL</label>
              <input
                placeholder="如 https://api.deepseek.com"
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.target.value)}
              />
            </div>
            <div className="form-field">
              <label>API Key（存入钥匙串）</label>
              <input
                type="password"
                placeholder="sk-..."
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
              />
            </div>
            <div className="form-field">
              <label>接口类型</label>
              <select
                value={wireApi}
                onChange={(e) => setWireApi(e.target.value as "responses" | "chat")}
              >
                <option value="responses">responses（原生）</option>
                <option value="chat">chat（/chat/completions）</option>
              </select>
            </div>
            <div className="form-field full">
              <label>默认模型 ID</label>
              <input
                placeholder="如 deepseek-chat"
                value={model}
                onChange={(e) => setModel(e.target.value)}
              />
            </div>
          </div>
          <div className="wizard-actions">
            <button className="btn ghost" onClick={() => setStep(1)}>
              上一步
            </button>
            <div style={{ display: "flex", gap: 10 }}>
              <button className="btn" disabled={testState === "testing"} onClick={testConnection}>
                {testState === "testing" ? "测试中…" : "测试连接"}
              </button>
              <button className="btn primary" disabled={busy} onClick={finishCustom}>
                {busy ? "配置中…" : "完成并切换"}
              </button>
            </div>
          </div>
          {testState !== "idle" && (
            <div
              className="hint"
              style={{ color: testState === "ok" ? "var(--green)" : "var(--red)" }}
            >
              {testState === "testing" ? "测试中…" : testMsg}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
