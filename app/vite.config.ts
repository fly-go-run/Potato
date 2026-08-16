/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function readProductVersion(): string {
  const versionFile = path.resolve(__dirname, "../src/potato/__version__.py");
  try {
    const match = /__version__\s*=\s*["']([^"']+)["']/.exec(
      readFileSync(versionFile, "utf8"),
    );
    if (match?.[1]) return match[1];
  } catch {
    // 前端单开时仓外可能没有 Python 包。
  }
  const packageJson = JSON.parse(
    readFileSync(path.resolve(__dirname, "package.json"), "utf8"),
  ) as { version?: string };
  return packageJson.version && packageJson.version !== "0.1.0"
    ? packageJson.version
    : "2.0.5";
}

// 后端同源托管：build 产物放 app/dist，可由 POTATO_WEB_STATIC_DIR 覆盖。
// dev 模式下 /api 代理到本地后端（默认 8088，与旧 console 一致）。
export default defineConfig(({ command }) => ({
  plugins: [react(), tailwindcss()],
  define: {
    __APP_VERSION__: JSON.stringify(readProductVersion()),
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
        // 桌面版后端端口随机，联调时用 POTATO_DEV_BACKEND 覆盖
        target: process.env.POTATO_DEV_BACKEND || "http://localhost:8088",
        changeOrigin: false,
        ws: true,
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
