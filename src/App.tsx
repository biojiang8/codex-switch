import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import type { StatusInfo, SwitchConfig } from "./types";
import SetupWizard from "./components/SetupWizard";
import Home from "./components/Home";
import Providers from "./components/Providers";
import Settings from "./components/Settings";

type Page = "home" | "providers" | "settings";

export default function App() {
  const [status, setStatus] = useState<StatusInfo | null>(null);
  const [config, setConfig] = useState<SwitchConfig | null>(null);
  const [page, setPage] = useState<Page>("home");
  const [toast, setToast] = useState<{ text: string; kind: "success" | "error" } | null>(null);
  const [ready, setReady] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [s, c] = await Promise.all([api.getStatus(), api.getConfig()]);
      setStatus(s);
      setConfig(c);
    } catch (e) {
      showToast(String(e), "error");
    }
  }, []);

  useEffect(() => {
    refresh().finally(() => setReady(true));
  }, [refresh]);

  const showToast = (text: string, kind: "success" | "error") => {
    setToast({ text, kind });
    setTimeout(() => setToast(null), 3500);
  };

  // 首次使用：无任何 provider → 向导
  const needsWizard = ready && config && Object.keys(config.providers).length === 0;

  if (!ready) {
    return (
      <div className="app" style={{ alignItems: "center", justifyContent: "center" }}>
        <span className="spinner" /> 加载中…
      </div>
    );
  }

  if (needsWizard) {
    return (
      <SetupWizard
        onDone={() => {
          refresh();
          showToast("配置完成！", "success");
        }}
      />
    );
  }

  const navItems: { id: Page; label: string }[] = [
    { id: "home", label: "首页" },
    { id: "providers", label: "Provider 管理" },
    { id: "settings", label: "设置" },
  ];

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="logo">
          <div className="logo-badge">⇄</div>
          <div>
            <h1>Codex Switch</h1>
            <div className="ver">v3.0 桌面版</div>
          </div>
        </div>
        <nav className="nav">
          {navItems.map((n) => (
            <div
              key={n.id}
              className={`nav-item ${page === n.id ? "active" : ""}`}
              onClick={() => setPage(n.id)}
            >
              <span className="dot" /> {n.label}
            </div>
          ))}
        </nav>
        <div className="sidebar-foot">
          ChatGPT 官方 / API 切换
          <br />
          Key 存 macOS 钥匙串
        </div>
      </aside>

      <main className="main">
        {page === "home" && (
          <Home status={status} config={config} onChanged={refresh} showToast={showToast} />
        )}
        {page === "providers" && (
          <Providers config={config} onChanged={refresh} showToast={showToast} />
        )}
        {page === "settings" && (
          <Settings config={config} onChanged={refresh} showToast={showToast} />
        )}
      </main>

      {toast && <div className={`toast ${toast.kind}`}>{toast.text}</div>}
    </div>
  );
}
