# Potato Computer Use 调研

日期：2026-08-16
修订：2026-08-16（按独立核对修正工具面口径、Codex 键盘/锁屏证据层级、DSH MCP 丢图）

来源分层：

| 层级 | 用来做什么 | 材料 |
|---|---|---|
| 官方文档 | 产品循环、平台能力、审批与 Locked use 的对外承诺 | [Codex Computer Use](https://learn.chatgpt.com/codex/computer-use)、[Computer Use API](https://developers.openai.com/api/docs/guides/tools-computer-use) |
| 本机组件 | 补充实现判断，不当成已动态验证的行为 | 插件 `1.0.1000717` / Sky `26.812.1000717`：skill、`SkyComputerUseService`、`SkyComputerUseClient`、`CUALockScreenGuardian`、`CodexComputerUseAuthorizationPlugin` |
| 开源代码 | 可复用的驱动与已知限制 | `trycua/cua` @ `2f5a2a6`（contract、focus_guard、no-foreground contract）、DSH `mcp-client` @ `47f94385` |
| 社区实现 | 对照，不代表官方 Harness | `Anionex/dsh-computer-use` |
| Potato 源码 | 我们现在有什么、MCP 会不会丢图 | `desktop_screenshot`、`browser_use`、`drivers/adapters/agentscope_tool.py` |

本机二进制分析只用于确认符号和进程形态，不把没跑过的路径写成事实。

---

## 0. 结论

这个方向值得做。最合适的方案不是复刻 Codex 的私有实现，而是：

> **Codex 的交互协议与治理方式 + Cua Driver 的后台驱动能力 + Potato 自己的权限和状态管理**

三家真正能用的 computer use，核心不是「截全屏再移动真实鼠标」，而是：

1. **按 app / pid / window 寻址**，不要操作「当前前台桌面」。
2. **优先 Accessibility 语义动作**（`AXPress` / `AXSetValue`），像素点击只是兜底。
3. **输入走进程/窗口定向投递**，不走全局 HID，不移动真实硬件鼠标。
4. **观察绑定动作**：先拍一版 AX 树，动作只能打这版树上的 index / token；过期就拒。
5. **Agent 光标是 overlay**，不是输入源。
6. **审批按 app 租约 + 高风险二次确认**，跟 shell 沙箱是另一条轴。

Cua 对「不抢焦点」的承诺是 **best-effort**，不是绝对保证。默认必须 background-only，禁止自动升到 foreground。

---

## 1. Potato 现状

| 能力 | 现在有什么 | 缺什么 |
|---|---|---|
| 看屏幕 | `desktop_screenshot`：`mss` 全屏；macOS 可选 `screencapture -w` 让用户点窗口 | 不能按 pid/window 后台抓 |
| 操作 GUI | 无 | 不能点、打字、滚、拖 |
| 网页 | `browser_use`（Playwright，ref 寻址） | 网页应继续走这条，不该交给桌面 CUA |
| 审批 | 通用 tool 审批 | 没有按 bundle id 的 read/control 租约 |
| 驱动接入 | 已有 MCP Driver / `DriverCard` | 还没接 desktop driver |
| MCP 图片 | `agentscope_tool._blocks_from_mcp_content` 把带 `mimeType` 的 data 收成 `DataBlock(Base64Source)` | 还要确认各 provider 真的把 DataBlock 送给视觉模型 |

Potato 的 MCP 适配器**不会**像 DSH 那样在投影阶段把图片打成占位符。接 `cua-driver` 时，截图进模型上下文这条路是通的；还差的是窗口级观察和定向输入。

---

## 2. 三者的真实差异

| 方案 | 观察方式 | 输入方式 | 是否干扰当前工作 | 关键特点 |
|---|---|---|---|---|
| Codex Computer Use | Accessibility Tree + 窗口截图 | app 定向的语义操作和坐标操作 | macOS 可后台；Windows 官方是前台占用 | 接口克制、状态绑定强、权限治理完整 |
| DeepSeek Harness | 官方仓库没有原生 Computer Use | 依赖外部 MCP / 社区插件 | 取决于外部驱动 | 官方 MCP 桥接会丢掉发给模型的图片 |
| Cua Driver | AX/UIA/AT-SPI + 单窗口截图 + CDP | AX、PID/窗口定向事件、CDP；必要时前台 HID | 默认 best-effort 不抢焦点 | 开源里最接近 Codex，工程细节扎实 |
| `dsh-computer-use`（社区） | AX + 可选窗口截图 | AX 优先，指针走 `SLEventPostToPid` | 指针默认不激活；**插件自己的**键盘默认会激活目标 | 租约和过期观察写得干净，只做 macOS |

OpenAI 文档里的循环是：模型输出操作 → harness 执行 → 截取新屏幕状态 → 回传给模型。macOS 插件支持用户在做别的事时后台跑。

---

## 3. Codex Computer Use（本机 26.812.1000717）

### 3.1 产品形态

不是 CLI 内置工具，是桌面 app 插件：

```
插件包   ~/.codex/plugins/cache/openai-bundled/computer-use/1.0.1000717/
启动器   bin/computer-use-client-launcher  →  exec SkyComputerUseClient mcp
Skill    skills/computer-use/SKILL.md      →  教模型用 node_repl + @oai/sky
原生服务 ~/.codex/computer-use/Codex Computer Use.app
           ├── SkyComputerUseService                         TCC 主体，LSUIElement
           ├── SkyComputerUseClient.app                      MCP / node_repl 客户端
           ├── CUALockScreenGuardian.app                     锁屏相关进程存在
           └── .../CodexComputerUseAuthorizationPlugin.bundle 官方 Locked use 用的授权插件
```

- Bundle id：`com.openai.sky.CUAService`
- Sky 客户端版权仍写 Software Applications Incorporated
- `bundledContentVariant: node-repl`：模型面对的是一个小 JS SDK，不是几十个 MCP tool
- 本机 IPC 通道名是 **`CodexComputerUseIPC-3`**（client / service / guardian 三端都能看到）。网上部分逆向文里的 `-1` 已过时
- 不能操作 Terminal / Codex 自己
- 网页优先走内置 Browser + Chrome 扩展

### 3.2 模型面

Skill 把动作收成带 `app` 的窄 API：

```ts
sky.list_apps()
sky.get_app_state({ app, disableDiff? })   // AX 树 + screenshot，默认 diff
sky.click({ app, element_index?, x?, y?, mouse_button?, click_count? })
sky.set_value({ app, element_index, value })
sky.type_text({ app, text })
sky.press_key({ app, key })                // 只打给指定 app，不能发全局快捷键
sky.scroll({ app, element_index, direction, pages? })
sky.select_text({ app, element_index, text, prefix?, suffix? })
sky.perform_secondary_action({ app, element_index, action })
sky.drag({ app, from_x, from_y, to_x, to_y })
```

死循环：`get_app_state` → 用本轮 `element_index` 动作 → 再 `get_app_state`。运行时自己 settle（约 1 秒，loading 最多再等 5 秒）。AX 残了才退到截图坐标。`get_app_state` 会后台拉起没开的 app。

这是 Codex 好用的一半：模型不用记 pid/window_id，也不用一次面对几十个工具。

### 3.3 原生层（本机符号，不是动态验证）

`SkyComputerUseService` / `CUALockScreenGuardian` 能看到：

| 符号 | 更稳妥的读法 |
|---|---|
| `ScreenCaptureKit` / `SCStream` | 窗口级截图，不必把窗口抬到前台 |
| `AXUIElement` | 语义树和控件操作 |
| `CGEvent` | 合成输入存在；**不等于**走全局 HID |
| `VirtualCursor` | 独立 Agent 光标 overlay |
| `SyntheticAppFocusEnforcer` | 改目标应用内部「当前控件」，不必把该 app 变成系统前台 |
| `SystemFocusStealPreventer` / `FocusStealSuppression` | 目标若把自己切到前台，尝试压回去 |

因此更准确的理解是：

> Codex 会改变目标应用内部的当前控件，但尽量不改变用户正在使用的系统前台应用，也不移动真实硬件鼠标。

`press_key` / `type_text` 在 skill 里写的是 **app-targeted**。把 Codex 说成「键盘默认激活目标应用」不成立——那是社区插件 `dsh-computer-use` 自己的 `keyboardPolicy: activate`，它声称对齐 Codex，但这是插件政策，不是 OpenAI 文档或本机 skill 的原文。

Cua / DSH 对 `SLEventPostToPid`、`SLPSPostEventRecordTo`、primer click 的逆向，和上面这组符号相容，但那些 SPI 名没有全部以明文出现在本机 client 的导出表里（service 通过 PrivateFrameworks / `dlopen` 用）。报告里把它们标成「开源侧对 Codex 路径的还原」，不当成本机已 `nm` 到的事实。

TCC 挂在 `Codex Computer Use.app` 上。系统录屏/辅助功能权限，和 Codex 内部「允许控制哪些 app」是分开的。

### 3.4 审批

1. **系统权限**：Screen Recording + Accessibility。
2. **App 批准**：第一次碰某个 app 弹卡，可 Always allow。Windows 落在 `$CODEX_HOME/config.toml` 的 `computer_use.windows.always_allowed_app_ids`。

Skill 里的 Confirmations Policy 按后果分级：Hand-off（改密码、转账）/ 动作时必须确认（不可恢复删除、签 Tos）/ 允许预批 / 不问（只读）。用户自己打的字算意图；网页和粘贴的第三方内容不能当授权。

### 3.5 Locked use（证据要降级）

官方文档描述了 Locked use：装 Authorization Plugin，在受信任的 CUA turn 里短暂解锁、盖住显示器，侦测到本地键鼠再锁回去。

本机可以确认的只有：

- `CUALockScreenGuardian.app` 存在
- `CodexComputerUseAuthorizationPlugin.bundle` 存在
- 官方文档写了上述行为

「自动短暂解锁再锁定」**没有**在本机动态验证。P2 之前不要当已实现细节抄。

---

## 4. DeepSeek Harness

### 4.1 官方没有 Computer Use

`deepseek-ai/deepseek-harness` 是 Cordis 插件运行时。标准档是文件 / shell / 搜索 / skill / subagent。**没有桌面 CUA。**

它可以在结构上挂 `cua-driver mcp`。但官方 [`@deepseek-ai/dsh-mcp-client`](https://github.com/deepseek-ai/deepseek-harness/blob/47f943859bef60e4160492346772ded9b24f765a/packages/mcp/mcp-client/README.md) 写明：

> Native/model rendering keeps the existing text projection: text blocks join with newlines while **image, audio, resource, and unsupported blocks become placeholders**.

所以：

- AX 文本驱动可以工作
- 截图进不了模型上下文
- 坐标兜底和视觉确认到不了 Codex 的水平
- 若模型本身不支持视觉，还得另挂 perception sidecar

社区插件不要和官方混：

| 项目 | 做什么 |
|---|---|
| `Anionex/dsh-computer-use` | macOS Accessibility-first 原生控制 |
| `Lum1104/dsh-browser` | 连已打开的 Chrome tab，走 DOM |

`dsh-computer-use` 的默认 `keyboardPolicy: activate` 是**该插件**的选择。它的价值在租约、observation TTL、`targetHandle` 重绑失败闭合，以及「1ms 采样光标和 frontmost pid」这套验收，不在「官方 Harness 已经会 CUA」。

---

## 5. Cua Driver

两套产品，别用错：

| 产品 | 场景 |
|---|---|
| **Cua Driver** | 驱动本机已经在跑的 app。后台、不抢光标。 |
| **Cua Sandbox + Lume** | VM / 容器里给 agent 一台电脑。 |

要的是 Driver。

### 5.1 驱动层（开源代码）

[no-foreground contract](https://github.com/trycua/cua/blob/2f5a2a6b9455e35dda73ca2d92aeae511672afad/docs/content/docs/concepts/the-no-foreground-contract.mdx) 自己写的是 **best-effort background**：

1. 优先 AX（`AXUIElementPerformAction` / `AXUIElementSetAttributeValue`）。
2. 键盘定向投给目标 PID，不是全局前台窗口。
3. 鼠标用窗口局部坐标，定向发给 PID/窗口。
4. 截图只抓指定窗口（macOS 上是 ScreenCaptureKit）。
5. Agent 光标是 overlay。
6. 目标若把自己切到前台，`focus_guard` 把原来的 frontmost 抢回来。
7. 浏览器可绑原生窗口 + CDP tab。

`focus_guard.rs` 是 Swift `FocusGuard.withFocusSuppressed` 的 Rust 移植，注释里的第三层就叫 `SystemFocusStealPreventer`——和本机 Codex service 里的符号同名。这是「Cua 最接近 Codex」的直接证据，不是类比。

升级梯子也是显式的：background AX → 同窗口像素 → **调用方**才允许 `delivery_mode: foreground`。

### 5.2 工具面：25 和 56 都是真的，层不同

| 层 | 数量 | 含义 |
|---|---|---|
| Portable SDK / contract manifest（`libs/cua-driver/contract`，`contract_version` 0.7.0） | **25** | 跨平台 session + 桌面循环 + cursor + `verify_state` 的 typed 切片 |
| 托管文档里的 live MCP（cua.ai，driver 0.20.0 生成页） | **56** | `list_apps` / `get_window_state` / `launch_app` / `page` / 录轨迹等平台完整面 |

给模型看哪一层都太胖。Codex 的模型面大约 10 个方法。Potato 要包的是 **25 那个 contract 量级里再收成 8 个 facade**，不是把 56 个 MCP tool 原样注册。

寻址比 Codex 更严：`pid` + `window_id` + `element_token` / `snapshot_id`。过期失败而不是点错。连用户已登录的 Chrome 要显式 `--grant existing-profile`。

权限三档启动时钉死：`standard` / `bounded` / `unrestricted`。macOS TCC 必须挂在 `CuaDriver.app` 上。

### 5.3 已知破功（必须写进产品文案）

- 游戏、Unity、Blender、部分 Canvas 只吃真实 HID，要显式切前台
- 最小化窗口有时不接受键盘提交
- 跨 Space 的 SwiftUI 窗口可能拿不到完整 AX 树
- Chromium 网页内容的合成右键可能被打成左键
- macOS PID 定向输入用了私有 SkyLight SPI，系统升级可能坏
- Windows 普通完整性进程打不进提权窗口（UIPI）；SSH Session 0 没有交互桌面

所以 API **默认禁止自动升级到 foreground**。只有调用方或用户明确同意，才允许这一次。

---

## 6. 对照

| 维度 | Codex Sky | 官方 DSH | dsh-computer-use | cua-driver | Potato 现在 |
|---|---|---|---|---|---|
| 开源 | 否 | 无 CUA | 社区 MIT，macOS | MIT，三平台 | — |
| 模型面 | 1 个 JS SDK，~10 方法 | 外部 MCP，图片变占位符 | ~11 个 tool，Skill 才露出 | contract 25 / live MCP 56 | 截图 + Playwright |
| 寻址 | `app` 名 / bundle | — | bundle + observation + handle | pid + window + token | 整屏 |
| 首选路径 | AX `element_index` | — | AXPress / AXValue | AX token | 无 |
| 系统光标 | 不移动（macOS） | — | 测过不移动 | 默认不移动 | 无操作 |
| 系统前台 | 尽量不改；有 FocusSteal 抑制 | — | 指针 preserve；键盘政策自选 activate | best-effort；可显式 foreground | — |
| 截图进模型 | 是 | 官方 MCP **否** | 取决于 DSH 投影 | 是（MCP image block） | Potato MCP **会保留** |
| App 审批 | Always-allow | — | read session / control turn | standard/bounded/unrestricted | 通用 tool 卡 |
| 状态绑定 | 每轮换 index，默认 AX diff | — | observation 过期即拒 | snapshot_id 过期即拒 | 无 |

Codex 好用，主要好在协议和治理。Cua 好用，主要好在开源驱动和 focus guard。官方 DSH 现在接 Cua 也做不到 Codex 体验，因为图到不了模型。Potato 没有这个坑。

---

## 7. Potato 该怎么接

### 7.1 不要做的

- 不要复刻 Sky / SkyLight / ScreenCaptureKit 细节。
- 不要用 `pyautogui` / 全局 `CGEventPost` / `cliclick`。
- 不要把 Cua 的 25 或 56 个工具直接暴露给模型。
- 不要用 CUA 代替 `browser_use`。
- 不要自动 `delivery_mode=foreground`。
- 不要把 DSH 社区插件的 `keyboardPolicy: activate` 当成 Codex 默认行为来抄。

### 7.2 架构

```
模型
  ↓
Potato computer facade          ← Codex 那种窄协议
  ├─ list_apps
  ├─ observe
  ├─ click
  ├─ set_value
  ├─ type_text
  ├─ press_key
  ├─ scroll
  └─ drag
  ↓
权限审批 / app lease / 状态 token / 高风险确认   ← Potato 自己的
  ↓
Cua Driver daemon               ← 后台驱动
  ↓
AX + 窗口截图 + PID 定向输入 + CDP
```

三层分开：

1. **Driver**：`cua-driver`。macOS 先装 `CuaDriver.app` 拿 TCC。走现有 `DriverCard`，不要让模型直接 `tools/list` 全量 MCP。
2. **Facade**：自己收成上面 8 个。`observe` 返回 AX + 可选窗口截图；动作必须带本轮 snapshot/token。默认 background。Cua 回报 `effect: unverifiable` 时原样回给模型，**不要**替它升 foreground。
3. **治理**：租约 key 用 bundle id。只读 observe 可记 session；click/type 默认 ask；付钱 / 删除 / 装软件走 ApprovalCard。Codex 四级政策当 skill 文案，不当唯一闸。禁止 CUA 点 Potato 自己的审批卡和终端。

### 7.3 分期

P0（macOS，background-only）：

- 接 `cua-driver` daemon，不把 live MCP 全量暴露
- 8 个 facade
- app lease + 总开关 + TCC 状态
- 网页继续 `browser_use`
- 截图走现有 DataBlock 路径，确认视觉模型真能看见

P1：AX diff、Agent 光标主题、Windows、显式一次性 foreground（用户点头才行）。

P2：登录态 Chrome 的 existing-profile grant、轨迹录制。Locked use 先不做。

### 7.4 验收（没有这条就不许宣传「不抢鼠标」）

独立采样，全程必须同时成立：

- 用户当前 frontmost PID 不变
- 实体鼠标坐标不变
- 当前 Space 不变
- 用户编辑器输入焦点不变
- 目标应用确实发生了预期状态变化

游戏 / canvas 破这条时，产品文案写「原生 AppKit / 多数 Electron 可后台；这类表面要显式前台」。

---

## 8. 和审批调研的衔接

`approval-mechanism-research.md` 改的是 shell / 文件。CUA 是第三条轴：

- 沙箱挡不住「点 iMessage 发送」
- Always-allow 必须按 bundle id
- AUTO 超时硬拒会让 CUA 整段废掉：只读可 AUTO，控制至少 SMART
- 禁止 CUA 操作 Terminal 和 Potato 自己

---

## 9. 本轮核对与勘误

相对第一稿：

| 原表述 | 修正 |
|---|---|
| Cua 暴露 56 个工具 | live MCP 文档是 56；**portable contract 是 25**。两层都不要原样给模型 |
| 官方 DSH / 社区插件混在一节里写实现 | 官方无 CUA；且官方 MCP **投影阶段丢图** |
| Codex 键盘默认会激活目标应用 | 不成立。那是 `dsh-computer-use` 的政策。Codex skill 是 app-targeted；service 里是 `SyntheticAppFocusEnforcer` + `SystemFocusStealPreventer` |
| Locked use「短暂解锁再锁定」写成已确认实现 | 官方文档 + guardian/plugin 二进制存在；**行为未动态验证** |
| 未写 IPC 通道 | 本机是 `CodexComputerUseIPC-3` |
| 未写 Potato MCP 是否丢图 | 现有 adapter **保留** image DataBlock |

---

## 10. 参考

- 本机 skill：`~/.codex/plugins/cache/openai-bundled/computer-use/1.0.1000717/skills/computer-use/SKILL.md`
- 本机 app：`~/.codex/computer-use/Codex Computer Use.app`（Sky 26.812.1000717，IPC `CodexComputerUseIPC-3`）
- https://learn.chatgpt.com/codex/computer-use
- https://github.com/trycua/cua/blob/2f5a2a6b9455e35dda73ca2d92aeae511672afad/docs/content/docs/concepts/the-no-foreground-contract.mdx
- https://github.com/trycua/cua/blob/2f5a2a6b9455e35dda73ca2d92aeae511672afad/libs/cua-driver/contract/README.md
- https://github.com/trycua/cua/blob/2f5a2a6b9455e35dda73ca2d92aeae511672afad/libs/cua-driver/rust/crates/platform-macos/src/focus_guard.rs
- https://github.com/deepseek-ai/deepseek-harness/blob/47f943859bef60e4160492346772ded9b24f765a/packages/mcp/mcp-client/README.md
- https://github.com/Anionex/dsh-computer-use
