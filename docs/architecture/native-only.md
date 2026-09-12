# 原生架构与旧实现退役原则

决策日期：2026-09-08。依据项目维护者的明确要求：只维护原生客户端，原有实现逐步废弃。

## 唯一维护目标

Potato 当前的产品开发、体验审查、缺陷修复和发布目标是 **Rust GPUI 前端 + 进程内 Rust potato-core 后端**。

| 路径 | 定位 | 后续工作原则 |
| --- | --- | --- |
| `native/potato-gpui/` | 当前 Rust 原生前端 | 界面、交互、状态与体验问题从这里检查和修复 |
| `native/potato-core/` | 当前 Rust 共享核心 | 业务逻辑、持久化、模型调用、工具与调度从这里检查和修复 |
| `app/`、`console/` | 旧 React/TypeScript、Tauri 客户端 | 逐步废弃，仅作历史行为、视觉与资源参考，不再默认开发或同步修复 |
| `src/potato/` 中的 Python 服务、CLI 与业务实现 | 旧运行时 | 逐步废弃，不再作为新功能或当前客户端缺陷的默认修改目标 |
| `native/potato-ui/` | 旧 Iced 原型 | 迁移参考，不是当前维护的原生前端 |
| `scripts/native/build-desktop.mjs` | 旧 React/Tauri 构建入口 | 不作为当前原生客户端的构建、测试或发布入口 |

“原生”不表示任何 Rust 界面都仍是维护目标，也不表示仓库里不能保留 TypeScript、Python 文件。打包脚本、提示词、图标及共享资源可以继续保留；当前应用运行时不依赖 Python 服务或 WebView。

## 开发与审查规则

1. 接到功能、体验或 bug 请求，先定位 `native/potato-gpui` 与 `native/potato-core` 的实际调用链。不能因旧代码更容易测试而改查旧前端。
2. 旧实现中的问题不自动视为当前产品的问题。需要在原生调用链中确认是否存在，再决定修复位置。
3. 默认只修改原生实现及其必要资源、测试和构建文件。除非用户明确要求，不再给旧 React/Tauri、Python 运行时或 Iced 版本同步实现功能、补兼容或修 bug。
4. 修复完成必须说明实际覆盖的客户端与验证范围。TypeScript 编译、React 测试或浏览器截图不能作为 GPUI 前端修复完成的证据。
5. 旧文档若与本决策冲突，以本决策和当前原生代码为准；旧文档只用于理解历史。

## 构建、测试与发布

构建与平台依赖以 [GPUI README](../../native/potato-gpui/README.md) 为准。常用入口：

```sh
cargo +1.96.1 test --locked --manifest-path native/potato-core/Cargo.toml
cargo +1.96.1 test --locked --manifest-path native/potato-gpui/Cargo.toml
cargo +1.96.1 build --locked --release --manifest-path native/potato-gpui/Cargo.toml
python3 native/potato-gpui/package.py
```

发布工作流为 `.github/workflows/gpui-release.yml`。按改动运行相关测试；涉及界面时，在 GPUI 客户端验证，明确区分自动测试与实机验收。

## 渐进退役

- 先统一入口文档与维护范围，停止默认维护旧实现。
- 移除旧文件前检查原生代码、打包和 CI 的真实依赖，先迁移仍被使用的共享资源。例如 GPUI 当前仍引用 Iced 目录中的流适配代码，以及旧目录中的图标、提示词等资源。
- 分批移除失去依赖的旧代码、构建入口与文档，并验证原生构建和数据迁移。
- 本决策不代表旧目录已经删除，也不代表所有旧功能已经迁移完成。此次只明确维护方向，不批量删除源码或用户数据。

## 对此前审查的纠正

2026-09-08 对记忆管理和定时任务的审查混入了旧 React 前端，并修改了旧 Python 定时任务管理器。这些修改不应算作 GPUI 前端修复。

其中，`potato-core` 的记忆 NUL 字符写入校验和星期范围解析修复属于当前原生共享核心；其余前端问题需要在 GPUI 中重新确认。已经完成的旧代码修改保留为现状，不据此继续扩展旧实现。
