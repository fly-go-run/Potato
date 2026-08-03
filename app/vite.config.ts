/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const packageJson = JSON.parse(
  readFileSync(path.resolve(__dirname, "package.json"), "utf8"),
) as { version?: string };

// 后端同源托管：build 产物放 app/dist，可由 QWENPAW_WEB_STATIC_DIR 覆盖。
// dev 模式下 /api 代理到本地后端（默认 8088，与旧 console 一致）。
export default defineConfig(({ command }) => ({
  plugins: [react(), tailwindcss()],
  define: {
    __APP_VERSION__: JSON.stringify(packageJson.version ?? "0.0.0"),
  },
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
  // Keep the ignored screenshot helper available to `vite` during local QA,
  // but never copy it into the backend-served production package.
  publicDir: command === "serve" ? "public" : false,
  build: {
    outDir: "dist",
    sourcemap: false,
    // Keep a visible budget for entry chunks. Markdown is loaded on demand
    // below, while route-level views already use React.lazy in App.tsx.
    chunkSizeWarningLimit: 350,
    rollupOptions: {
      output: {
        manualChunks: {
          "react-vendor": ["react", "react-dom", "react-router-dom"],
        },
      },
    },
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
  },
}));
