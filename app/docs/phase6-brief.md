# Phase 6 任务包：技能与插件管理（执行者：Codex）

办公用户需要管理 agent 的能力（docx、pdf、browser 等技能和插件）：查看、启停、安装、卸载。
约束不变（只动 `app/`、不动 `tokens.css`、不新增依赖、文案全走 zh/en、语义色类）。
**执行纪律：联调每条路径验证一遍即可，清理后核对一次即止，不做重复核查；
如某项验证受阻，记录到报告里就继续，不要空转。**

## 入口与信息架构（按此实现）

- 设置页新增「技能与插件」区块（分区卡片风格与现有一致）：一行摘要
  （已启用 x / 共 y 个技能 · z 个插件）+「管理」按钮 → 跳转 `/skills`。
- `/skills` 独立页面：页头（标题 + 副标题 + 右上「添加」主按钮）+
  顶部 segmented 切换「技能 | 插件」（样式同设置页主题按钮组）。
- 侧栏**不加**入口（保持克制），路由懒加载独立 chunk。

## 技能 tab

API（`src/qwenpaw/app/routers/skills.py`，prefix `/api/skills`；以真实响应为准先 curl）：
- `GET /api/skills` → 工作区技能 `{name, emoji, description, enabled, version_text, tags, source, ...}`。
- `POST /api/skills/{name}/enable|disable`（会触发 agent 后台 reload）；`DELETE /api/skills/{name}` 删除。
- 技能池 `GET /api/skills/pool`（内置/共享库，含 docx 等）；池 → 工作区的导入路径读源码确认
  （`/pool/download`、`/pool/import-builtin` 等，选对的用）。
- Hub：`GET /api/skills/hub/search?...`、`POST /hub/install/start` → 轮询
  `GET /hub/install/status/{task_id}`（完成/失败/可取消）。
- ZIP 上传：`POST /api/skills/upload`（或 pool/upload-zip，读源码选适合工作区安装的）。

UI：
- 搜索过滤框 + 列表行：emoji + 名称 + 单行 description（line-clamp，注意不要与 block 冲突）+
  version 淡显 + 启停 Switch（复用 crons 的开关样式）。切换时行内 loading，失败回滚并 Banner 提示。
- 行点击 → 详情抽屉：完整描述、tags、来源、版本；底部次要区放删除（带确认）。
- 「添加技能」Dialog 三个来源 tab：**技能池**（列表 + 已装标记 + 导入按钮）/
  **Hub 搜索**（关键词搜索 + 安装 + 进度轮询）/ **上传 ZIP**。

## 插件 tab

API（`src/qwenpaw/app/routers/plugins.py`，prefix `/api/plugins`）：
- `GET /api/plugins` 已装列表；`GET /api/plugins/catalog` 官方目录（含 installed 标记）；
- `POST /api/plugins/install` `{source: 路径或URL}`、`POST /api/plugins/upload`（ZIP）、
  `DELETE /api/plugins/{plugin_id}`（即时生效，无需重启）。
- 若源码中存在插件级启停 API 就加 Switch；没有就不做开关（卸载即关闭），不要造假开关。

UI：
- 已装列表行：名称 + 描述 + 版本 + 提供的工具数（如响应里有）+ 卸载（确认）。
- 「添加插件」Dialog：**官方目录**（已装标记 + 安装）/ **URL 安装** / **上传 ZIP**。

## 交付

- 单测：启停乐观更新与回滚、hub 安装状态机（轮询 pending→done/failed）、
  目录已装标记合并逻辑。
- `npm test` + `npm run build` 通过；真实联调：启停一个技能（如 docx）并确认列表状态变化、
  从池导入一个技能再删除、插件目录加载。**每项一遍即止。**
- 报告 `app/docs/phase6-report.md`（简洁：逐条状态 + 已知限制，不超过 80 行）。
