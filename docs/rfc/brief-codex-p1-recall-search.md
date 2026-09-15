# P1：E2B 检索替换为 Worker 内实现（仅 Worker）

依据：`docs/rfc/mobile-cloud-convergence.md` v2 第 3 节；另附带 P2 需要的两个 Worker 语义固定测试（第 4 节末）。你评审过这份 RFC。

约束：
- 仓库根 `/Users/liuxu/lifeProjects/Potato`，分支 `feat/process-track-codex`，**不要 commit、stash、revert 任何你没改的文件**。
- 只改 `native/potato-worker/src`、`native/potato-worker/test`、`native/potato-worker/README.md`、`native/potato-worker/wrangler.jsonc`（仅 `limits`）。不碰 iOS、Rust。
- 不改 SSE 事件名、不改候选选择与翻页逻辑、不改 R2 格式。
- 完成后写 `docs/rfc/p1-recall-search-report.md`：改动、规格对应、真实验证输出、CPU 测量数字、跳过项。

## 规格

### 1. `src/recall-search.ts`

导出 `searchConversations(input: unknown, signal: AbortSignal): Promise<{ sources: RecallSource[]; more: boolean }>`，签名满足 `RecallExecutor`。逐条对应 `recall.ts` 里 `RECALL_SEARCH_CODE` 的 Python 语义：

- 输入 `{query, start, end, offset, conversations:[{id,title,revision,messages:[{id,role,text,date,version?}]}]}`。
- 规范化 `norm(s) = s.normalize('NFKC').toLowerCase()`；`q = norm(query).trim()`；`terms = q.split(/\s+/u).filter(Boolean)`（Python `str.split()` 无参按任意 Unicode 空白切分）。
- 过滤：`start` 非空且 `m.date < start` 跳过；`end` 非空且 `m.date >= end` 跳过（字符串比较，输入已是 ISO）。
- 计分：`text = norm(m.text)`；`score = q && text.includes(q) ? 10 : terms.filter(t => text.includes(t)).length`；`q` 非空且 `score === 0` 跳过。
- 行：`{...m, conversation: c.id, title: c.title, revision: c.revision, score}`。
- 排序：`(score, date)` 降序（Python `sort(key=(score,date), reverse=True)`，注意稳定性：相同键保持原顺序的**反转**——用 `rows.sort((a,b) => b.score - a.score || (b.date > a.date ? 1 : b.date < a.date ? -1 : 0))` 后，对完全相同键的元素顺序与 Python 的差异按等价测试结果处理，若不等则补稳定序号反转）。
- 分页：`o = offset ?? 0`；`sources = rows.slice(o, o+8)`，每条 `text` 截前 4000 个**码点**：`Array.from(text).slice(0, 4000).join('')`；`more = rows.length > o + 8`。输出对象里**不含** `score`。
- 取消：每处理 50 条消息 `signal.throwIfAborted()`。

### 2. 接线

- `recall.ts`：删除 `e2bRecallExecutor` 与 `RECALL_SEARCH_CODE`，删除 `@e2b/code-interpreter` 的 import（`sandbox.ts` 的仍保留）。`RecallExecutor` 类型保留，`close` 可选字段可以去掉。
- `index.ts`：recall 前置条件改为只要 `env.RECALL_BUCKET`（不再要求 `E2B_API_KEY`）；执行器直接用 `searchConversations`；去掉围绕它的 `SANDBOX_RATE_LIMIT` 计费包装。`RECALL_RATE_LIMIT` 照旧。
- `search-chat.ts` 的 `recall?.execute.close?.()` 调用若类型上不再存在则删除。

### 3. 等价测试

- 新建 `test/fixtures/recall-corpus.json`：12 个对话，覆盖中英混排、全角标点、Unicode 空白（如 U+3000、U+00A0）、一条超过 4000 码点且含 emoji/代理对的消息、跨 3 个日期、同分并列。
- 用本机 `python3` 运行原 `RECALL_SEARCH_CODE`（从 git 里的旧版本或本次删除前复制到 `test/fixtures/recall-search.py`）对 6 组查询（整句命中、多词、空 query+日期范围、offset=8、大小写差异、全角/半角）生成期望输出 `test/fixtures/recall-expected.json`。**把 python 脚本和生成命令保留在 fixtures 目录**，报告写明 Python 版本。
- `test/recall-search.test.ts`：6 组查询逐字节比较（`JSON.stringify` 排序键后比较）。若某组因 `casefold` vs `toLowerCase` 差异不等，在测试里把该差异显式列为已知例外并断言其余相等；报告里列出。
- 现有 `test/recall.test.ts` 中依赖 E2B mock 的用例改为注入 `searchConversations`，断言不变。

### 4. CPU 测量与配置

- 在 `test/recall-search.test.ts` 加一个非断言的测量用例：用 10 个各 150 KB 的合成对话连续搜索 8 次，`process.cpuUsage()` 差值换算毫秒，`console.log` 输出。报告写数字。
- `wrangler.jsonc` 加 `"limits": { "cpu_ms": 30000 }`。若 `npx wrangler deploy --dry-run` 因计划不支持报错，去掉并在报告里写明。
- README「跨对话检索与记忆」段：改为说明检索在 Worker 内执行，不再需要 E2B。

### 5. P2 需要的两个固定语义测试（`test/recall.test.ts`）

- `sources` 为空数组的记忆在 `validMemories` 中恒有效。
- `POST /v1/recall/memory` 的 `forget: true` 请求若缺 `text` 字段返回 400；`text: ""` 通过。

验证：
```sh
cd native/potato-worker && npm run check && npm test && npx wrangler deploy --dry-run 2>&1 | tail -3
```
