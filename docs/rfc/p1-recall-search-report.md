# P1：Worker 内跨对话检索实施报告

日期：2026-09-13。分支：`feat/process-track-codex`。依据 `mobile-cloud-convergence.md` v2 第 3 节及第 4 节 Worker 固定语义要求。

## 改动与规格对应

| 规格 | 实现与验证 |
| --- | --- |
| Worker 内执行 | 新增 `src/recall-search.ts`，导出异步 `searchConversations(input: unknown, signal: AbortSignal)`；测试将其赋给 `RecallExecutor`。输入由原有 RecallSession 和存储校验链提供。 |
| 规范化、分词 | NFKC + `toLowerCase()`，query trim，`/\s+/u` 分词；覆盖中英混排、U+3000、U+00A0、全角标点和大小写。 |
| 日期、计分 | start 包含、end 排除，ISO 字符串比较；整句命中 10，否则按命中词数计分，非空 query 的零分行跳过。 |
| 排序、结果分页 | score/date 降序，每页 8 条，原 offset/more 公式；并列顺序和两页拼接有固定测试。 |
| 输出、截断 | 保留消息字段和可选 version，增加 conversation/title/revision，不输出 score；原文按 4000 码点截断，emoji 长消息进入 Python 等价比较并有单独边界断言。 |
| 取消 | 入口及每处理 50 条消息调用 `throwIfAborted()`，计数包含日期过滤掉的消息；测试覆盖预先取消及第 100 条消息检查时取消。 |
| 接线 | 删除 recall.ts 的 E2B import、执行器和 Python 常量，保留 RecallExecutor，移除 close 字段和 search-chat.ts 的 close 调用。index.ts 只要求 RECALL_BUCKET，直接注入新执行器，移除检索的 SANDBOX_RATE_LIMIT 包装。RECALL_RATE_LIMIT 未改。 |
| 回归 | 原检索结果替身改为真实 searchConversations，保留来源验证、SSE、失败/取消不提交记忆等断言。原 Python 安全检索测试改用新执行器，保留断言。仅适用于 E2B 的创建/复用/销毁计数测试被替换为 HTTP 入口测试：无 E2B key、沙箱限额拒绝时检索仍成功，沙箱限额调用数为 0。 |
| P2 固定语义 | 空 sources 的未遗忘记忆在无对话、同步、编辑、排除及移除索引后均有效；主动遗忘后仍无效。真实 POST handler 测试验证 forget 缺 text 返回 400，传空字符串及原 revision 返回 200。 |
| 配置、文档 | wrangler.jsonc 仅增加 `limits.cpu_ms: 30000`；README 跨对话检索段改为 Worker 内执行、只依赖 R2。 |

候选选择、conversation_offset、10 个对话/1.5 MB 预算、SSE 事件名、R2 格式均未改。`sandbox.ts` 及 E2B 依赖保留供 Python 工具使用。除用户指定的本报告外，本次只写入允许的 Worker 路径；未修改 Rust/iOS，未 commit、stash 或 revert 工作树文件。

## Python 等价证据

环境：macOS arm64，Node `v26.7.0`，Python `3.9.6`，Wrangler `4.131.1`。

`test/fixtures/recall-corpus.json` 含 12 个对话、24 条消息、3 个日期、同分同日期消息、混合文字与可选 version。其中长消息含 3990 个 emoji，超过 4000 码点。六组查询为：整句、多词 Unicode 空白、空 query + 日期范围、offset=8、大小写、全角/半角。

`recall-search.py` 从删除前常量复制，已用 `git show HEAD:native/potato-worker/src/recall.ts` 核对原文完全一致（文件增加结尾换行）。生成器仅将原脚本的 open 重定向至固定输入，不改变检索代码。

从仓库根运行，命令也保留在 fixtures 生成脚本 docstring 中：

```sh
python3 --version
python3 native/potato-worker/test/fixtures/generate-recall-expected.py
```

实际输出：

```text
Python 3.9.6
Generated 6 Python reference results
Original Python matches HEAD; expected fixture regeneration is byte-identical
Expected SHA-256: 38629987888f44c1f423600ef789b600b4efcfa5308f133c9e69d72c359dc7f8
```

比较使用递归排序对象键后的 JSON.stringify 字符串，数组顺序保持不变。五组完全相等；大小写组显式列出一个允许的 casefold 例外：query `sTrAsSe` 在 Python 命中 `Straße camera 轻便`，JS 不命中。测试锁定该唯一消息的 ID、Python 结果数量 1、JS 数量 0，剔除此条后完整对象相等，另断言 CaMeRa 与 camera 结果完全相同。

两处规格澄清：

- 原 Python 输出包含 score，而 P1 明确要求无 score。生成器在输出边界仅删除 score，再保存期望；并非声称与含 score 的原始 stdout 字节完全相等。新执行器有显式无 score 断言。
- Python `reverse=True` 对相同键仍保持输入顺序，未反转并列元素。实际 Python 对比和并列分页测试均通过，因此没有增加序号反转。

## 真实验证输出

执行用户要求的命令：

```sh
cd native/potato-worker && npm run check && npm test && npx wrangler deploy --dry-run 2>&1 | tail -3
```

最终运行退出 0，输出摘录：

```text
> check
> tsc --noEmit

> test
> node --test test/*.test.ts

recall CPU: user=57.512 ms system=4.136 ms total=61.648 ms; 8 searches, 10 x 150000 bytes
✔ Python equivalence: phrase
✔ Python equivalence: unicode-whitespace-terms
✔ Python equivalence: empty-date-range
✔ Python equivalence: offset-eight
✔ Python equivalence: case
✔ Python equivalence: fullwidth
✔ memories with empty sources remain valid independently of conversation state
✔ POST recall memory forget requires text, and accepts an empty string
✔ chat recall needs only R2 and never charges sandbox rate limits
ℹ tests 80
ℹ suites 0
ℹ pass 80
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
ℹ todo 0
ℹ duration_ms 1645.014291
env.CLOUD_ACCESS_AUD ("1bf05234bd56ae3508e5235764cd8b062d75d...")            Environment Variable

--dry-run: exiting now.
```

另以 `set -o pipefail` 重跑 dry-run，将完整输出保留到 `/tmp/potato-p1-wrangler-dry-run.log`，确认 Wrangler 自身退出 0，未发现 WARNING/ERROR。打包输出 `Total Upload: 962.05 KiB / gzip: 192.29 KiB`。没有计划不支持错误，因此保留 limits 配置。`git diff --check -- native/potato-worker` 通过。

首次受限环境运行中，Miniflare 因 `listen EPERM: operation not permitted 127.0.0.1` 无法启动；改为允许本地监听的执行环境后全量通过。首次新增截断测试还因长消息不在第一页而失败，已修正为独立 emoji 查询，并调整固定长消息日期使其进入 Python 等价比较第一页；最终结果如上。

## CPU 测量与跳过项

测量用例在生成数据之后取 `process.cpuUsage()`，对 10 个序列化后各 150,000 UTF-8 字节的合成对话连续搜索 8 次，再取 user/system 差值；无性能阈值断言。最终全量运行累计 **61.648 ms**，平均 **7.706 ms/次**，累计扫描输入约 12 MB。该数据是本地 Node 进程 CPU（含期间运行时/GC 开销），不是墙钟、Miniflare invocation CPU 或生产分位数。

以下未执行：

- 实际部署、真实模型/E2B 调用、iOS/GPUI 验收及 RFC P0 生产检查；本任务只实施 Worker 和 dry-run。
- Workers Paid 账号计划确认；dry-run 打包成功不能证明线上计划支持该 CPU 配置，上线前仍需确认。
- 部署后一周 invocation CPU p95/p99 和超限率；尚未部署，不能据本地均值宣称生产门槛通过。
- 全 Unicode 字符集穷举；本次固定语料覆盖规格列出的空白/全角/emoji 和已知 casefold 差异。

取消按规格采用同步扫描中的周期检查；它不会主动让出事件循环，因此本地计数测试不等于证明扫描过程中由异步事件触发的取消可即时送达。原 R2 来源复核中的既有 UTF-16 截断逻辑保持不变，本次码点等价保证针对新 searchConversations 输出。
