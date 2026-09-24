# 代码示例执行策略 · Worker 部署

2026-09-17 已部署 `potato-iphone-api`，版本 `8d8d69b8-6452-4c49-9ae9-4173cea1c8c0`，回读确认 100% 流量。无须重发 iOS。

本次从上一线上版本 `51e229f1-6acf-4cf6-b620-98a831882d61` 的源码基线隔离发布，仅 `src/code-tool.ts` 和 `src/search-chat.ts` 提示词变化。工作树内未发布的远程 outbox 改动未包含；隔离目录 `/tmp/potato-policy-release-20260917` 的 remote.ts 及其测试与上一版本摘要一致。保留线上变量和密钥，不改绑定或迁移。

规则：写/解释/修改/演示代码默认不运行；明确要求执行或目标需要实际计算、文件处理/产物时才调用沙箱。代码与对应输出一起展示时先代码后输出；未执行输出标为预期/示意。详见 [策略](../execution-policy.md)。

## 验证

- 隔离发布包全量 91 项回归通过，TypeScript 检查和 Wrangler dry run 通过。
- 真实 DeepSeek V4.1 Flash 请求：“写一段 Python 代码”——0 次沙箱调用。
- 真实请求解释 `print(sum([1, 2, 3]))`——0 次沙箱调用。
- 明确请求沙箱执行 `print(17 * 19)`——1 次调用成功，实际 stdout 为 323，回复代码在前、输出在后。
- 全部请求仅含合成数据；使用既有 QA 会话，凭据未写入记录。模型行为具有概率性，这三条检查不是所有输入的强制运行拦截保证。

[部署回读](deployment.json) · [真实验证](live-verification.json) · [源码摘要](source-sha256.json) · [测试摘要](tests.json)

完整日志：`/tmp/potato-policy-release-tests.log`、`/tmp/potato-policy-deploy.log`。源码摘要在打包/发布后核验一致。此前隔离环境外运行测试因本地监听权限失败，获得所需本地监听权限后在隔离基线全量通过。
