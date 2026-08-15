# 任务：P1-c 整体对抗审查（只审查 + 报告，不改代码）

审查 Tool Runtime P1 的完整实现（后端 P1-a + 前端 P1-b），产出
`docs/rfc/tool-runtime-review-c.md`，P0/P1/P2 分级，每条 file:line 证据。

范围：`git log 4dfc4fa5..HEAD` 的全部实现 commit
（ef03fd59/72a4cd60/2d7548fb/075fd76c 后端；7c7dfba7 前端 app/src）。
基准：`docs/rfc/tool-runtime.md` r2 + `impl-p1a-report.md`。

## 重点

1. 契约一致性：后端产出的每个 kind 字段与 RFC §1.2 表、与前端
   `app/src/lib/toolMeta.ts` 消费的键名是否逐一对上（尤其
   bytes_written/size_bytes/additions/deletions/exit_code/sandboxed）。
   任何一边拼错键名都是静默失效,单测抓不到跨端漂移。
2. 前端回落正确性：meta 存在但字段缺失、meta 畸形、历史无 meta 三态
   下 FileToolCard/fileChanges/ShellToolCard 的行为;
   `isSuccessfulArtifactPair` 的 ok=false 否决会不会误伤 running 态。
3. 传输矩阵实测口径：envelope 终态透传、取消无 qp、历史回填、
   split 保护深拷贝——测试是否真的覆盖了断言的场景。
4. 行为等价：content blocks 文本是否零改动（模型视角不变）。
5. 4KB/50 条边界、qp 终态唯一性在 coordinator 聚合路径上的真实性。

## 纪律

- 不挑战 RFC 既定裁决与 backlog。只报会导致 bug/静默失效/返工的问题。
- 只写报告，不改任何代码。
