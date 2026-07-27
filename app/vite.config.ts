/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// 后端同源托管：build 产物放 app/dist，由 QWENPAW_CONSOLE_STATIC_DIR 指向。
// dev 模式下 /api 代理到本地后端（默认 8088，与旧 console 一致）。
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    port: 5174,
    proxy: {
      "/api": {
        // 桌面版后端端口随机，联调时用 QWENPAW_DEV_BACKEND 覆盖
        target: process.env.QWENPAW_DEV_BACKEND || "http://localhost:8088",
        changeOrigin: false,
      },
    },
  },
  build: {
    outDir: "dist",
    sourcemap: false,
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
});
