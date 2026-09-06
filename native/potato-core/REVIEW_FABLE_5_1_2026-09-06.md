# Fable 5.1 high — Rust history review (2026-09-06)

## Authorized follow-up fixes (2026-09-06)

The user subsequently authorized concrete fixes without a broad redesign. These
are now implemented and verified:

- Damaged histories are isolated and surfaced through catalog `history_error` and
  `/api/native/history-health`. Healthy conversations continue to load.
- A deletion marker independent of journal parsing/capacity allows damaged-session
  removal, interrupted-deletion recovery, and later same-ID reimport.
- New session creation uses a stable ID and adopts a published orphan after SQL
  failure; recovery preserves conflicting orphan archives instead of crashing.
- A bounded last-session cache and shared inspection remove repeated hydration and
  serialization on unchanged histories. Metadata stamps include same-size edits,
  replacement and sidecar changes. Startup loads catalog metadata only.
- Normal model file writes cannot modify the runtime history subtree. The short
  retrieval paths remain in each turn even when memory previews are unchanged.
- Derived index errors are nonfatal; unchanged index files are not rewritten.
  Coexisting job directories merge disjoint IDs and preserve/report conflicts.

The full suite now reports 138 passed / 1 ignored, and both desktop crates compile.
The original review and its initial verification notes below are preserved as a
historical record; statements such as "no production source was modified" describe
that earlier review turn. Cross-conversation full-text search remains a linear scan;
no new search service, database index or background artifact GC was added. Details
and remaining boundaries are in [TRANSCRIPT_STORAGE.md](TRANSCRIPT_STORAGE.md).

## Invocation and scope

User-requested Claude Code CLI review, using `--model claude-fable-5-1 --effort high`.
The CLI confirmed the requested main model and completed successfully. Only Read,
Glob and Grep were enabled; there were no permission denials or spawned subagents.
The review took approximately 9 minutes. CLI-reported estimated total cost was
USD 5.8848 (including a small internal Haiku helper call).

Review scope: authoritative JSONL history, text sidecars, migration, catalog
recovery, project navigation, context/steering integration, exports and shell
archives. All 16 recorded source-file hashes remained unchanged during review
and local reproduction. No production source was modified by this review task.

## Parent verification and triage

The findings below are the independent reviewer's original output. They are not
all accepted unchanged. Parent verification establishes:

1. **Confirmed by executable reproduction: one corrupt session blocks runtime
   startup.** With two imported sessions, append a malformed complete JSONL line
   to only one, then reopen Runtime. Startup fails before the healthy session can
   be accessed. Preserve the damaged data and surface a per-session recovery error
   instead of failing the whole application.
2. **Confirmed by executable reproduction: published orphan plus same-session
   retry poisons catalog recovery.** Force generated README publication to fail
   after journal initialization by replacing README.md with a directory. The
   import returns an error and its SQL transaction rolls back. Remove the induced
   obstruction, import another ID for the same session, then reopen Runtime.
   Startup fails with `UNIQUE constraint failed: chats.session_id`. New-chat
   creation has the same journal-before-index/commit ordering. Moving SQL upsert
   before file publication alone does NOT close the crash window: recovery and
   retry must reconcile the already-published session identity without discarding
   either journal silently.
3. **Confirmed by code inspection: repeated full-history I/O.** Archive::append
   loads and hydrates all old records, then serializes them again. Steering checks
   also replay history; Runtime holds the shared Store mutex. Search and startup
   amplify this. No latency benchmark was run, so the original review's concrete
   subsecond/seconds estimates are not established measurements. An incremental
   projection must invalidate correctly on external changes and cross-process
   writes; file length alone is not a sufficient cache identity.
4. **Confirmed by code inspection: ordinary model file writes can target the
   authoritative archive when the default project is the workspace.** It is
   intentionally readable, but currently receives ordinary project write policy.
   Keep file-based discovery while treating runtime-owned journals and artifacts
   separately from model-edited notes. A shell cwd restriction does not confine an
   unsandboxed shell's absolute-path accesses; do not present it as an OS boundary.
5. **Confirmed by code inspection: every catalog change rewrites all navigation
   groups; conversation body search replays candidate histories.** These are real
   scaling concerns, with impact still needing measurement.
6. **Confirmed by code inspection: the short history-location guidance is gated
   with the memory fingerprint.** Once that old environment message is summarized,
   subsequent unchanged snapshots do not reannounce the paths. Retrieval can still
   recover old data through recall_history, but the navigation bootstrap should
   remain available without relying on an old summary retaining paths.

Corrections/qualifications to the original review:

- A normal append rejects content beyond the 256 MiB bound before publishing it.
  Merely approaching the bound therefore does not, by itself, prove startup will
  fail. The separate problem remains that append-based deletion can fail if there
  is insufficient headroom for the tombstone; corruption or missing artifacts
  also prevent that deletion path.
- Pre-upgrade jobs shown in this implementation did not originally store
  `stdout_path` / `stderr_path`. The ordinary migration is not demonstrated to
  contain stale values as P2-5 asserts. Recomputing paths on load would still be a
  defensive improvement for moved archives or imported state containing paths.
- Both old/new job directories do trigger a startup error if they coexist, but
  the claimed downgrade trigger is not established: the immediately older core
  rejects schema 3 before opening jobs. Backup restoration or another producer
  can still produce the conflicting directories.
- Orphan artifact cleanup, partial import visibility after failed publication,
  and queued UI frame rendering were not independently reproduced in this task.

## Reproduction output

```text
CONFIRMED corrupt single chat blocks all startup: Corrupt transcript line 3: key must be a string at line 1 column 2
Induced index publication failure: Is a directory (os error 21)
CONFIRMED orphan/retry session conflict blocks startup: UNIQUE constraint failed: chats.session_id
```

Temporary reproduction source and raw CLI stream are in
`/tmp/potato-fable-review-20260906/`. The reproduction calls the public Runtime
API against temporary installations; it does not touch the user's runtime data.
Existing 122 passing tests remain useful, but did not cover the two reproduced
failure paths. This review did not rerun the full suite or implement fixes.

## Original Claude Code review (unaltered)

审查完成。以下是基于实际代码（非文档）的发现，按优先级排列；每项给出触发场景、影响与最小修复。

## 必须修复（P1）

**P1-1 历史日志和作业归档放在模型可写的 workspace 内，且运行时把路径直接告知模型。**
- 位置：`/Users/liuxu/lifeProjects/Potato/native/potato-core/src/store.rs:76-77`（history_root 在 workspace 下）、`store.rs:258-278`（jobs 从私有的 `<root>/jobs` 迁到 `workspace/history/jobs`）、`store.rs:309-310`（把 transcript/index/jobs 路径写进模型上下文）、`projects.rs:115-121`（check_public_path 对 workspace 全部放行）、`tools.rs:277-291` 与 `approval.rs:183-195`（项目内且非 sensitive 的写操作在 AUTO 下免审批）、`approval.rs:53-78`（sensitive 列表不含 history）。
- 场景：默认配置 AUTO + workspace-write，且未选择项目时 `project_dir()` 就是 workspace（`projects.rs:123-133`）。模型从 runtime_context 拿到 `.../workspace/history/sessions/<id>/transcript.jsonl`，调用 write_file / edit_file / append_file 即可无审批改写正在使用的日志或 artifacts，也能删除 job 的 stdout 归档。一次注入或“好心整理”写坏一整行，就触发 P1-2。改动前消息正文在 root 下的 SQLite 中，jobs 也在 root 下，均被 check_public_path 拒绝。
- 修复：把 `<workspace>/history` 视为运行时私有区：在 `PreparedWrite::prepare`、shell cwd 检查和 `sensitive()` 中拒绝该前缀的写入；或把归档移到 `<root>/history` 并在 check_public_path 中只放行读取。

**P1-2 任一会话不可读就让 `Store::open` 失败，整个应用无法启动，且无应用内恢复途径。**
- 位置：`store.rs:423-474` recover_catalog 对每个会话执行 `archive.read` 全量水合并逐项 `?` 传播；`store.rs:459-468` 目录缺失直接返回 500；`store.rs:84-85` 与 `lib.rs:149-150` 直接向上抛出。
- 触发：a）某会话原始或水合体积达到 256 MiB（`transcript.rs:278-311`），共享 session 的定时任务会无限增长（`scheduler.rs:354-358`）；b）任一完整行损坏（外部编辑、P1-1、云同步半截文件）；c）任一 artifact 缺失；d）用户清理或备份恢复时没有 `workspace/history/sessions`，而 SQLite 仍有目录项。
- 影响：启动即失败，设置与凭据都不可用；到达上限后 `delete_chat`（`store.rs:612` 先要 append tombstone，而 append 先全量 load）和 `Runtime::start`（`lib.rs:298` deliver_steering 先 rows）也都失败，坏会话既删不掉也发不了消息。
- 修复：recover_catalog 只解析 envelope 中的 session/deleted 事件，不做 text_refs 水合、不套水合上限；单会话失败转为目录项标记（如 `spec["unreadable"]=原因`）并继续启动，同时对被标记会话拒绝 append 以保留“绝不静默重建”的语义；delete_chat 在无法 load 时允许直接持锁删除目录。

**P1-3 每次 append / 转向检查 / 读取都全量回放并水合整个会话，且在全局 Store 互斥锁内执行。**
- 位置：`transcript.rs:335-362` append 调用 `load` 读完整个文件、解析每行、打开并读取每个 artifact，再在 345-348 重新序列化所有旧记录；`store.rs:656-661` has_steering 走 `rows` 全量读；调用点 `tool_execution.rs:39,43,125`、`tools.rs:315`、`approval.rs:236`、`questions.rs:37`、`model.rs:122,228`、`compaction.rs:28`、`steering.rs:70`。
- 每次工具调用的全量回放次数约为 7 到 8 次（call 帧 append、两到三次 has_steering、result append、deliver_steering、context history、assistant append）。每次都要重新读取正是 sidecar 设计想移出热路径的大文本。`Runtime.store` 是 std Mutex，回放与 fsync 期间阻塞 tokio 工作线程和所有 UI 请求。旧实现是 SQLite 点查和单行插入。一个几十 MB 的长会话，每次工具调用会退化到亚秒到数秒级，启动时 recover_catalog 还会把所有会话再读一遍。
- 修复（最小）：在 Store 内按 archive id 缓存已水合的 rows 及 (文件长度, 水合总量)，因为进程内所有写入都经过 Store；append 时若持锁后文件长度等于缓存长度且末字节为 `\n`，跳过 `load`，否则再全量校验；has_steering 用缓存中的 queued 计数。另外 `transcript.rs:361` 对已存在 inode 的追加不需要目录 fsync，可去掉一次 F_FULLFSYNC。

## 应当修复（P2）

**P2-1 孤儿日志携带重复 session_id 会让启动永久失败。**
- 位置：`store.rs:541-572` 新会话路径先 `initialize`（566）发布文件，再写 SQL、`refresh_indexes`、commit（568-570）。若 568 之后失败（如写索引文件 ENOSPC、commit 失败），文件留在磁盘而 SQL 回滚；用户重试发送时 `ensure_chat` 生成新 id，产生第二份同 session_id 的日志。下次启动 recover_catalog（456）依次 upsert，第二份触发 `UNIQUE constraint failed: chats.session_id`，此后每次启动都失败。
- 修复：save_chat 先在 Immediate 事务内做 SQL upsert（失败自动回滚），再 initialize；recover_catalog 遇到 session_id 已属其他 id 时跳过并标记，而不是 `?` 传播。

**P2-2 新旧 jobs 目录并存时 `Runtime::open` 直接失败。**
- 位置：`store.rs:269-273` 返回 409，`lib.rs:150` 传播。场景：升级后用旧版本跑过一次（旧版会在 `<root>/jobs` 建目录），再升级即无法启动，只能手工合并。修复：按 job id 逐目录搬移不存在的条目，不再整体 409。

**P2-3 refresh_indexes 每次目录变更都重写并 F_FULLFSYNC 全部项目索引和 README。**
- 位置：`store.rs:477-539`；调用点 569、618、820、87。每次新建会话或切换项目都要对历史上访问过的每个项目文件各做一次 `sync_all`（macOS 上为 F_FULLFSYNC）。修复：只重写内容变化的分组；生成型导航文件可以不 fsync。

**P2-4 会话列表搜索对每个会话做全量回放。**
- 位置：`api.rs:161-163`，每次 `?q=` 都对所有会话 `history(id,false)`。配合 P1-3 在多会话大历史下每次击键都是全盘读取。修复：先只匹配会话名，正文搜索改为显式操作或使用缓存。

**P2-5 迁移后的旧 job 状态里 stdout_path/stderr_path 仍指向旧私有路径。**
- 位置：`jobs.rs:93` 创建时写绝对路径，`store.rs:273` 只 rename 目录不改 state.json，`jobs.rs:185-186` 把这些字段返回给模型。模型按路径 read_file 会得到 403 “Native runtime data is private”（`projects.rs:117`）。修复：`Jobs::open` 中按 `self.root` 重写这两个字段，或从模型可见状态中删除。

## 可选（P3）

- **P3-1 压缩后模型丢失历史索引路径。** `model.rs:90-97` 把 history_guidance 并入受 memory 指纹门控的快照，只在指纹变化时附带；该条用户消息被摘要覆盖后，模型不再拥有 transcript 与项目索引路径，与文档“运行时会宣布路径”的承诺不符。可把这段短路径块放到门控之外。
- **P3-2 孤儿 artifacts。** `transcript.rs:352-355, 388-391` 在 pack 中先写 artifact，随后的体积检查或写入失败不会清理，文件永久残留。
- **P3-3 失败的导入在下次启动被静默采纳。** `store.rs:816-817` 先发布文件再写 SQL，中途失败时 API 返回错误，但 recover_catalog 会把已发布的日志重新加入目录。
- **P3-4 启动时 `lib.rs:298` 投递的转向帧被丢弃**，UI 仍显示为 queued，仅影响显示。

## 未能验证的部分

我没有运行测试，也无法查看 git HEAD 中旧的 `messages` 表结构，因此迁移 SQL（`store.rs:330-349`）与 schema 1/2 的实际列名是否一致只能依据代码中重建的 DDL 推断。Windows 上 `Archive::remove`（`transcript.rs:410-411`）持有 `journal.lock` 句柄时执行 `remove_dir_all` 的行为、以及 macOS 上 F_FULLFSYNC 的实际延迟，均未实测。路径遏制（UUIDv5、symlink 拒绝、artifact 名校验）、事件回放顺序、转向一次性投递和 torn-tail 保留的逻辑，我核对后没有发现缺陷。
