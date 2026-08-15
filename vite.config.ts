import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 开发标准配置：固定端口，禁用 clearScreen
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
