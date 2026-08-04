# 工作包：彻底删除内置 QA Agent

## 背景与决策

内置 QA Agent（`QwenPaw_QA_Agent_0.2`，产品答疑机器人）在新前端形态下已不可达：新前端（`app/`）不发 `X-Agent-Id` 头、所有会话只走 `default` agent；QA agent 的渠道配置为空，也不挂任何 IM 渠道。它唯一的代价是每次启动多 bootstrap 一个完整 workspace（启动时间 + 常驻内存）。用户已拍板：**彻底删除**——代码、模板、技能、md 模板、文档全清，并为存量用户配置做一次性清理迁移。

约束：

- **不要 commit**。仓库当前有大量其他未提交改动（`app/` untracked、多处 M），完成后只报告改动清单。
- 不要动与本任务无关的文件。
- `guidance` 技能是通用技能，**保留**；只删 `QA_source_index`。
- 旧 console（`console/src`）里的 `X-Agent-Id` / 多 agent 切换是通用多智能体机制，**不动**。
- `website/public/release-notes/` 是历史记录，**不改**。

## A. 后端代码删除

1. `src/qwenpaw/constant.py`（约 206–217 行）：删除 `BUILTIN_QA_AGENT_ID`、`BUILTIN_QA_AGENT_NAME`、`BUILTIN_QA_AGENT_SKILL_NAMES`、`LEGACY_QA_AGENT_ID` 及相关注释。

2. `src/qwenpaw/agents/templates.py`：
   - 从 `SUPPORTED_AGENT_TEMPLATES` 移除 `QA_AGENT_TEMPLATE = "qa"`，删常量与 `QA_TEMPLATE_DESCRIPTION`。
   - 删 `build_agent_template` 中 `template_id == QA_AGENT_TEMPLATE` 分支（约 114–131 行）及相关 import（`build_qa_agent_tools_config`、`BUILTIN_QA_AGENT_NAME`、`BUILTIN_QA_AGENT_SKILL_NAMES`）。
   - `get_workspace_md_template_id`（约 52–56 行）：集合只留 `LOCAL_AGENT_TEMPLATE`。注意：存量用户可能有 `template_id="qa"` 的自建 agent 落盘配置，此函数收到未知 id 必须继续优雅返回 `None`（现逻辑天然满足，别改成抛错）。

3. `src/qwenpaw/config/config.py`：删 `build_qa_agent_tools_config()`（约 2191 行起）。

4. `src/qwenpaw/app/migration.py`：
   - 删 `ensure_qa_agent_exists()`、`_do_ensure_qa_agent()`、`_apply_legacy_qa_disable_for_migration()`。
   - `_fallback_active_agent_id`（约 742 行）：候选序列 `(BUILTIN_QA_AGENT_ID, "default")` 改为只有 `"default"`（函数保留，新迁移还要用）。
   - **新增一次性清理迁移** `remove_builtin_qa_agent_profiles()`（见 §B）。

5. `src/qwenpaw/app/_app.py`：
   - 137 行处 `ensure_qa_agent_exists()` 调用改为调用新的 `remove_builtin_qa_agent_profiles()`，import 同步改。
   - 约 408 行 `start_all_configured_agents(..., ready_agent_ids=("default",))`：去掉 `ready_agent_ids` 实参（该机制整体移除，见下条）。

6. `src/qwenpaw/app/multi_agent_manager.py`（`start_all_configured_agents`，约 660–760 行）：
   - core phase 从 `("default", BUILTIN_QA_AGENT_ID)` 收敛为只有 `"default"`，删 `BUILTIN_QA_AGENT_ID` import。
   - **整体移除 `ready_agent_ids` 参数及其分支逻辑**——它当初只为"healthz 不等 QA"而加，QA 删掉后 core==default，语义退化为：`on_core_ready` 在 default 启动完成后触发。更新 docstring。

7. `src/qwenpaw/app/routers/workspace.py`：删 `BUILTIN_QA_AGENT_ID` import（37 行），约 669 行 `or ("qa" if agent_id == BUILTIN_QA_AGENT_ID else None)` 回退整个去掉（直接传 `agent_config.template_id`）。

8. `src/qwenpaw/cli/init_cmd.py`：删 `ensure_qa_agent_exists` import 与调用及 `click.echo("✓ Builtin QA agent workspace ensured")`（约 179、235 行两处）。

9. `src/qwenpaw/cli/agents_cmd.py`：模板列表来自 `list_supported_agent_templates()`，会自动少掉 `qa`，确认无 qa 硬编码即可。

## B. 存量用户一次性清理迁移

在 `migration.py` 新增 `remove_builtin_qa_agent_profiles()`，启动时调用（替换原 `ensure_qa_agent_exists` 的位置）：

- 用**字符串字面量**（常量已删）：`"QwenPaw_QA_Agent_0.2"` 和 `"CoPaw_QA_Agent_0.1beta1"`。
- 从 `config.agents.profiles` 中移除这两个 id（存在才处理）。
- 若 `config.agents.active_agent` 指向被移除的 id，用 `_fallback_active_agent_id` 重新落到 `default`。
- **不删除磁盘上的 workspace 目录**（用户数据安全），移除时 `logger.info` 打出 workspace 路径，提示用户可自行删除或将其重新注册为自定义 agent。
- 幂等：无事可做时不写 config；有变更才 `save_config`。

## C. 资产删除

- `src/qwenpaw/agents/skills/QA_source_index-en/`、`QA_source_index-zh/` 两个目录整体删除。
- `src/qwenpaw/agents/md_files/qa/` 目录整体删除。
- 检查 PyInstaller spec（`scripts/pack-tauri/qwenpaw.spec`）等打包脚本是否按目录名显式收集这些路径，有则同步清理（如果是整目录通配收集则不用动）。

## D. 测试

- `tests/unit/app/test_multi_agent_manager_startup.py`：重写。删除依赖 `BUILTIN_QA_AGENT_ID` 的用例（`test_core_ready_waits_for_enabled_qa`、`test_opt_in_core_ready_does_not_wait_for_qa` 等），核心语义改为：core phase 只有 default；default ready 即触发 `on_core_ready`；custom agents 在 core 后有界并发启动。保留并适配其余用例。
- `tests/unit/cli/test_cli_agents.py`：删 qa 模板相关用例（约 217、225–284 行，`test_agents_create_qa_template_uses_template_defaults` 等）及 `BUILTIN_QA_AGENT_SKILL_NAMES` import。
- `tests/integration/test_agents.py`：删 `BUILTIN_QA_AGENT_ID` import 与断言（约 54 行，改为断言 `default` 在列表中即可）。
- 为 §B 的清理迁移**新增单测**：含 QA profile 的 config → 移除且 active_agent 正确回落；不含 → 不写 config（幂等）。

## E. 前端

- `app/src/lib/skillPresentation.ts:7`：删 `qa_source_index` 一行（技能已不存在，避免死文案）。确认没有测试断言该 key。

## F. 文档

- `website/public/docs/persona.en.md` 与 `persona.zh.md`：删"Built-in QA Agent / 内置 QA 智能体"整节（约 198–213 行）。
- `website/public/docs/multi-agent.zh.md:756` 及 en 版对应处：提到"QA Agent"的示例改为其他专长示例（如代码审查 Agent），不留内置 QA 的说法。
- 其余 `website/public/docs/` 里 grep `QA` 的真实内置 QA 提及一并清理；release-notes 不动。

## G. 验证（全部必须绿）

```bash
uv run pytest tests/unit -q
uv run pytest tests/integration/test_agents.py -q   # 如环境不支持 integration，报告说明
cd app && npx tsc --noEmit && npx vitest run && cd ..
```

残留扫描（预期只剩：migration.py 里的清理迁移字面量、release-notes、历史 report/brief 文档）：

```bash
grep -rn "BUILTIN_QA\|LEGACY_QA\|build_qa_agent\|QA_AGENT_TEMPLATE\|ensure_qa_agent\|QA_source_index\|QwenPaw_QA_Agent\|CoPaw_QA_Agent" \
  src tests app/src console/src website/public/docs scripts
```

改动过的 Python 文件跑一遍 pre-commit（本仓库有 pylint 钩子）：`pre-commit run --files <改动文件>`。

## 产出

写报告到 `app/docs/qa-agent-removal-report.md`：改动文件清单、迁移行为说明、测试结果、残留扫描输出、任何有意跳过的项及理由。**不要 git commit。**
