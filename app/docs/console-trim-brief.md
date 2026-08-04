# Brief: console/ 裁剪到 Tauri 壳(删除旧 Web 界面)

## 背景

前端已绿地重写到 `app/`,所有 `/console` 静态资源分发路径(wheel_build.sh、install.sh、CI e2e、桌面 PyInstaller spec)都只用 `app/dist`。`console/` 里的旧 React Web 应用(pages/layouts/plugins 等)已无任何运行时入口,只剩三种存在感:桌面打包时 `tsc -b` 白白编译它、nightly 跑它的 vitest、CodeQL 扫它。

`console/` 目前唯一活着的东西是 **Tauri 桌面壳**:
- `src-tauri/`(Rust 壳 + 打包配置)
- 构建走 `npm run build:tauri-bootstrap`,入口 `tauri.html` → `src/tauri/bootstrap.tsx`
- bootstrap 依赖链(已核实,自包含):`src/tauri/**` + `src/i18n.ts` + `src/locales/*.json` + `src/contexts/ThemeContext.tsx`。样式只有 `src/tauri/BackendLoadingPage.module.less`,不依赖 `src/styles`。

## 目标

删除 `console/src` 下旧 Web 应用的全部代码,只留 Tauri 壳及其依赖链。**零行为变化**:`build:tauri-bootstrap` 产物应与删除前等价。

## 保留清单

- `console/src-tauri/` 整个目录,不动
- `console/tauri.html`
- `console/src/tauri/`(含 `backendRuntime.test.ts` 等测试)
- `console/src/i18n.ts`、`console/src/locales/`(不要清理 locale 里旧应用的字符串,不值得)
- `console/src/contexts/ThemeContext.tsx` + `ThemeContext.test.tsx`
- `console/src/test/`(vitest setup/mocks;删除旧应用后如有 mock 文件不再被任何测试引用,可一并删,如 chat-mock、react-window-mock 等,以 `npm run test:run` 通过为准)
- `console/src/vite-env.d.ts`
- `console/vite.config.ts`、`tsconfig*.json`、`eslint.config.js`、`package.json`、`public/`(tauri.html 引用了 `/online.svg`)

## 删除清单

- `console/src/pages/`(Chat、Agent、AppCenter、Coding、Control、Inbox、Login、Settings 全部)
- `console/src/layouts/`
- `console/src/plugins/`
- `console/src/components/`
- `console/src/api/`
- `console/src/hooks/`
- `console/src/stores/`
- `console/src/utils/`(若 tauri 链有引用则只删无引用部分,以 tsc 为准)
- `console/src/constants/`、`console/src/styles/`(同上,以 tsc 为准)
- `console/src/contexts/` 里除 ThemeContext 外的:`ApprovalContext`、`DesktopUpdateContext`(旧应用的更新 UI;新前端在 `app/src/components/desktop/DesktopHostBridge.tsx` 有自己的实现)
- `console/src/App.tsx`、`console/src/main.tsx`、`console/src/monacoSetup.ts`
- `console/index.html`(旧 Web 入口)

## package.json 调整

- 删除失效 scripts:`build`、`build:prod`、`build:test`、`preview`、`preview:prod`、`preview:test`(它们构建的是已删除的旧应用)
- 保留:`dev`(tauri.conf.json 的 beforeDevCommand 用 `vite --mode tauri` 走 tauri.html)、`build:tauri-bootstrap`、`test`、`test:run`、`test:coverage`、`format*`、`lint`
- 依赖清理**保守做**:只删删除后明显不再被任何保留代码引用的大件(例如 monaco-editor、react-router-dom、echarts 之类,如果确实无引用)。注意 **antd 必须保留**(CloseWindowPrompt 在用)、`@tauri-apps/api` 保留、react-i18next/i18next 保留。每删一个都要过一遍验收命令;拿不准就不删,不要追求删干净。

## 明确不要动的

- `app/` 下任何文件
- `console/src-tauri/`
- `scripts/`、`.github/workflows/`(nightly 的 console vitest job 继续有效——壳还剩测试;CodeQL paths 也不用改,console/src 仍存在)
- 后端 `src/`

## 验收(全部必须过)

```bash
cd console
npx tsc -b                       # 干净
npm run test:run                 # 剩余测试全绿
npm run build:tauri-bootstrap    # 成功,dist-tauri 产出正常
npm run lint                     # 干净(如报错仅限被删文件的残留引用,修掉)
```

另外全仓 grep 确认无残留引用(排除 dist、node_modules、docs、.reference-shots):

```bash
grep -rn "pages/Chat\|src/layouts\|src/plugins" console/src console/*.ts console/*.html
```

完成后给出:删除文件数/行数统计、验收命令输出摘要、依赖删了哪些没删哪些及原因。**不要 commit**,留给用户统一提交。
