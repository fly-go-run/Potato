# Rust / GPUI 流式 Markdown 接入调研

调研日期：2026-09-06。结论：优先正确接入现有 GPUI Kit 0.6.0 的 TextViewState 增量接口，同时修复正文布局；mdstream 可作为后续更严格的增量语义层备选。暂不替换 UI 框架，也不直接引入 Zed 内部 Markdown crate。

## 当前链路与证据

Potato media.rs → gpui_kit::component::text::TextView → gpui_base::TextView / TextViewState → markdown-rs 的 to_mdast → GPUI 原生节点。

当前 Cargo.lock 锁定 gpui-kit/base/component 0.6.0 和 markdown 1.0.0。markdown-rs 是 CommonMark/扩展语法解析器，不是自制的简陋文本解析。项目问题分为三个层次：

1. chat.rs 的执行过程预览在超过 64 字或两行后，直接用 div.child(narration).line_clamp(2)，绕开 Markdown 渲染。换解析器不会消除这条路径。
2. media.rs 用 TextView::markdown(id, 全文快照)。组件虽按元素身份缓存状态，但文本变化后调用 set_text；本地 gpui-base 0.6.0 state.rs:282 走 full replace。不能说每个 render 都重建状态，也不能说它完全没有异步/合并机制。
3. gpui-base 0.6.0 state.rs:294 已有 push_str 增量接口；parse_content:790 复用早期块，只重新解析末块及新增文本。后台任务合并更新并丢弃旧 revision 的结果。该路径仍会处理/复制文档源字符串，且长的未闭合单块仍可能反复解析，不能承诺严格线性性能。

官方 retained-state 文档：[TextView](https://gpui-kit.com/base/text-view)。解析器：[markdown-rs](https://github.com/wooorm/markdown-rs)。本地 registry 中的 0.6.0 源码是本次 API 判断依据，避免拿主分支新 API 套到旧版本。

## 候选方案

| 方案 | 角色与流式能力 | Potato 接入判断 |
|---|---|---|
| [GPUI Kit TextViewState](https://gpui-kit.com/base/text-view) | 原生排版、选择、链接、代码/表格；本地版本提供 push_str | 首选，已有依赖，改应用状态与更新方式即可；仍须修宽度与正文父容器 |
| [Latias94/mdstream](https://github.com/Latias94/mdstream) | 无界面的流式语义/状态引擎，稳定节点身份与变更通知 | 可接，但不是完整渲染器；需要把 Content IR 映射成 GPUI 节点或适配现有渲染器 |
| [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) / [comrak](https://github.com/kivikakk/comrak) | CommonMark/GFM 解析器；pull event stream 不等同可持续追加 token 的 UI | 可作为解析底座，但缓存、尾部状态、排版、选择与滚动都要自行接；仅更换它们不能直接修当前体验 |
| [Zed markdown](https://github.com/zed-industries/zed/tree/main/crates/markdown) | GPUI 原生 Markdown，源码有 append/replace、后台解析 | UI 相近但不是独立低耦合组件。Cargo.toml 依赖 language/theme/settings/ui/util 等工作区 crate；license 明确 GPL-3.0-or-later，与 GPUI 框架自身的 Apache-2.0 不同。当前 Potato 标注 Apache-2.0，不建议直接复制或作为即插即用依赖 |
| [egui_markdown](https://github.com/membrane-io/egui_markdown) | egui 控件，提供 heal(true) 处理未闭合流式语法、布局缓存 | Rust 方案存在，但控件属于 egui，不能直接塞进 GPUI 元素树 |
| [egui_commonmark](https://docs.rs/egui_commonmark/latest/egui_commonmark/) | egui Markdown viewer，缓存及可视区支持 | 同样有框架边界；其 scrollable 路径文档还明确提示静态内容假设，不宜当作现成 LLM 增量方案 |
| [codewandler/markdown](https://github.com/codewandler/markdown) | 增量 parser + ANSI/ratatui renderer | 偏终端，README 明示仍在建设，未完整覆盖 CommonMark；不作为此次 GPUI 替换首选 |

GPUI Kit 为 Apache-2.0；mdstream 为 MIT/Apache-2.0；egui_markdown 为 MIT/Apache-2.0；comrak 为 BSD-2-Clause。这里记录仓库声明，不展开法律判断。

## mdstream 的版本风险

本次 docs.rs 可读的发布文档展示 0.3.0，而仓库 main 已描述不兼容的 0.4 架构：从 committed/pending splitter 改为 StreamEngine + mdstream-protocol Reducer + typed Content IR。没有可靠核实到 crates.io 的 0.4 发布状态，因此不能把 main README 的依赖片段当成已发布可用版本。

若做此路线验证，应锁定发布版本或明确 commit，避免将 0.3 示例和 0.4 API 混用。main 明确将 layout、scrolling、token pacing 和 accessibility 留给宿主；引用修正、节点稳定性有专门机制，但不等于接入后自动修好跳动、换行和选择。

## 建议的接入顺序

1. 每个 session/message/content-part 对应稳定的 Entity<TextViewState>，不要以会变化的数组下标作为唯一身份。
2. 收到已归并的文本快照：若完全相同则不更新；若是原文前缀追加则仅 push_str(delta)；若回放/修正替换则 set_text(full)。保留协议原文，不能把显示用的语法补全写入历史。
3. 生成中和完成后用同一个 TextView::new(&state)、同一个固定宽度约束的正文容器。正文不经过两行预览，工具/明确 commentary 可另行折叠。
4. 应用按显示节奏批量更新；已有组件后台合并机制仍保留。聊天只用外层滚动，不要给每条消息再加 scrollable(true)。
5. 对迟到的 reference definitions 等跨块语义，单纯重解析末块不保证与完整文档一致。最终需要完整校正策略；set_text(相同文本)会直接返回，不能把同值 set_text 当成强制重解析。应单独设计完成时校正，再测阅读锚点是否稳定。
6. 用尾部未闭合粗体、链接、代码围栏、Setext 标题、列表紧凑/松散变化、GFM 表格、引用定义等检验增量和完整解析的一致性。若现有组件尾部策略不足，再试 mdstream，而不是先重写整个渲染层。

## 已做验证与边界

api_probe.rs 是独立的编译探针：TextViewState::markdown → snapshot-prefix 检查 → push_str / set_text → TextView::new。使用本地已构建的 gpui_kit rlib，以 rustc +1.96.1 --edition=2024 --crate-type=lib --emit=metadata 检查，通过（exit 0）。未修改 Cargo 依赖、未将探针接入应用。

这个验证只证明当前版本 API 可以集成，不是性能或视觉验收。尚未比较候选的 FPS、内存、长流选择稳定性、跨块引用语义。上一轮截图检查已证明生成过程容器与完成态换行有差异，那是应先修的应用集成问题。

建议验收矩阵：中文 2k/20k/100k 字；1/8/64 字符块输入；短正文、长单段、长代码、表格；800/1080/宽窗口；边选字边生成、上滚阅读、完成及 A→B→A 重连。记录输入响应、UI 帧耗时、尾部可见延迟、内存和最终解析一致性。
