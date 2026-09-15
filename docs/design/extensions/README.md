# 原生扩展可用性修复（2026-09-15）

范围：Rust potato-core + GPUI。没有修改旧 Python、React 或 Iced 实现。

## MCP

入口：侧栏「技能」→「MCP 服务」。

- 添加、编辑、删除服务；编辑器沿用草稿保护和删除确认。
- 配置编辑器提供 stdio / HTTP 模板。服务器标识使用字母、数字、下划线或连字符。
- 保存后点击「发现工具」，成功后工具进入模型调用目录；支持服务器和逐工具启停。
- 编辑时凭据显示为 `********`，保留该值会沿用已保存凭据；从 `env` / `headers` 对象删除键会移除凭据。
- 发现本地服务器工具会执行其启动命令。MCP 调用继续经过原有审批；AUTO 可由模型审查，不增加服务器整体免审规则。

stdio 示例：

```json
{"name":"Local tools","transport":"stdio","command":"npx","args":["-y","<server-package>"],"env":{},"enabled":true}
```

HTTP 示例：

```json
{"name":"Remote tools","transport":"streamable_http","url":"https://example.com/mcp","headers":{},"enabled":true}
```

核心按 runtime / 服务器复用连接，同一服务器请求串行执行，避免重复启动。更换传输配置或凭据、停用和删除服务会使旧连接失效；退出 GPUI 主动关闭连接。请求取消、超时或传输错误时丢弃连接，下一次调用重新连接，不自动重放可能已经产生副作用的调用。等待连接的请求会重新核验配置和工具启用状态。未改变的工具定义在重新发现时保留工具标识；定义改变时使旧标识失效。

## 技能包

入口：「技能」→「导入 ZIP」。现有手填 Markdown 入口保留。

- 包内必须恰好有一个 UTF-8 `SKILL.md`，可位于单层或多层包目录下。
- 支持 UTF-8 脚本、JSON、Markdown 引用及二进制 assets；二进制内容在 SQLite 中使用 Base64 保存。
- ZIP 上限 20 MB、200 项；单资源 2 MB，解压总内容 10 MB，`SKILL.md` 128 KB。
- 拒绝路径越界、绝对路径、反斜线/冒号路径、符号链接及重复文件。忽略 macOS 元数据文件。
- frontmatter 使用 YAML 解析，支持 BOM、CRLF、引号、注释、`>` 和 `|` 多行文本。启动时修复已保存技能的描述，不覆盖技能正文或启停状态。
- `read_skill` 支持 UTF-8 资源及 `./references/example.md`；拒绝越界路径和二进制文本预览，文本预览上限 128 KB。

### 运行包内脚本

模型先通过 `read_skill` 阅读指引和脚本，再使用原有 Shell 工具：

```json
{"command":"python3 \"$POTATO_SKILLS_DIR/my-skill/scripts/example.py\"", "skills":["my-skill"]}
```

每条命令仅复制明确请求、已启用的技能包到私有临时目录，通过 `POTATO_SKILLS_DIR/<name>` 访问；最多 16 个包、总计 20 MB。Windows 使用 PowerShell 的 `$env:POTATO_SKILLS_DIR` 语法。解释器与依赖必须已安装，不在导入时执行或安装。命令仍使用现有 Shell 审查、网络与文件隔离规则。输出应写到会话项目；临时副本在命令结束时清理。资源摘要参与执行环境指纹，包内容变化不会复用旧的正向审查缓存。

## 验证

最终工作区验证：core 192 项通过、11 项原有忽略；GPUI 68 项通过。core / GPUI Clippy（`-D warnings`）通过，GPUI 调试构建通过。工作区另有并行的附件/PDF等改动，测试数量包含这些既有/并行测试，不将其计为本次扩展修复。

- 技能测试覆盖 YAML/CRLF、路径规范化、二进制 ZIP 往返、资源复制、禁用、资源变化摘要，以及真实 Shell 审批后执行与临时目录清理。
- MCP 测试覆盖 stdio/HTTP、惰性发现、加密/掩码、多次调用仅启动一次、重发现标识稳定、配置变更拒绝旧调用和停用清理。
- GPUI 实体测试使用隔离的真实 core，覆盖创建/编辑服务、保存状态及掩码凭据保留。
- macOS 原生窗口验收：保存本地模拟 MCP 服务，发现 `echo`，关闭单个工具；从文件选择器导入含脚本、二进制资源及 CRLF 多行描述的 ZIP，确认技能列表与描述正确。
- Windows 实机、真实第三方 MCP 服务器、OAuth 未在此次验收范围内。

文件系统自动扫描、技能市场和通用插件系统仍未实现。
