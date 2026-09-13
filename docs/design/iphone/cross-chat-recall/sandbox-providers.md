# 个人历史文件空间：沙箱提供商比较

调研日期：2026-09-13。仅官方文档调研，未创建付费资源、迁移个人文件或运行性能测试。服务能力不等于当前 Potato 已接入，也不等于账号已获相应权限。

**建议将 Cloudflare Sandbox + R2 与阿里云沙箱列为两条重点验证路线。** 前者更贴近现有 Worker 架构；后者适合验证国内部署能否减少跨境等待。E2B 可以继续使用，存储层也可独立选型，不必为跨对话功能立即更换执行提供商。

| 提供商 | 官方已确认的文件能力 | 对 Potato 的意义与限制 |
| --- | --- | --- |
| Cloudflare Sandbox | `mountBucket()` 挂载 R2 或 S3 兼容存储，支持 `prefix`、`readOnly`；R2 binding 模式不需把真实存储凭证放入沙箱。 | 可按用户挂载历史目录，原始文件只读，记忆经服务接口更新；接入现有 Worker 较顺畅。挂载仍有网络文件访问开销。 |
| Daytona | S3 支撑的 FUSE Volumes，可用 `subpath` 挂载个人目录，沙箱删除后保留数据。 | 官方直接覆盖每用户/每租户持久空间；FUSE 不适合当数据库磁盘，共享区域当前列出 US/EU。 |
| Modal | Sandboxes 可挂 Volumes 或 CloudBucketMounts；支持子路径，卷更新需考虑 commit/reload。 | 适合复用同一批文件做分析。Volumes v2 仍为 Beta，官方不建议用于唯一的重要数据；同文件并发写需控制。 |
| 阿里云 | AgentRun 有实例级 NAS/OSS 挂载；官方推荐升级 FC 云沙箱，升级指南提供 OSS 前缀、只读配置及 E2B SDK 接入示例。 | 国内路线值得验证；需要核对 FC 具体地域、账号开通、模板和现有 TypeScript 代码解释器功能兼容性。不能把“兼容 E2B”理解为所有能力直接替换。 |
| E2B（现有） | 原生 Volumes；另有云存储挂载与暂停恢复。 | 继续复用成本最低。原生 Volumes 私有测试、仅 US/EU、无只读挂载等限制，见主方案中的官方核验。 |

能力来源：[Cloudflare 挂载](https://developers.cloudflare.com/sandbox/guides/mount-buckets/)、[Daytona Volumes](https://www.daytona.io/docs/en/volumes/)、[Modal 沙箱文件](https://modal.com/docs/guide/sandbox-files)、[Modal 卷语义与 v2 状态](https://modal.com/docs/guide/volumes)、[阿里云 NAS](https://help.aliyun.com/zh/agentrun/sandbox-supports-instance-level-dynamic-attachment-of-nas-test-invitation-1)、[FC 升级与 OSS 挂载](https://help.aliyun.com/zh/agentrun/agenrun-sandbox-upgrade-fc-cloud-sandbox-scheme)、[E2B Volumes](https://docs.e2b.dev/volumes)。

## Cloudflare 在本项目中的具体方案

沿用 Worker 做登录校验和工具调度，新增 R2 作为账号级文件存储；执行侧新增 Sandbox 适配器。现有 E2B 的模板、超时取消、文件产物和代码解释器行为需要逐项对应，不能只替换一个 URL。

```text
iPhone 增量同步
    ↓
Worker：确定用户、校验版本与删除记录
    ↓
R2：users/<user-id>/conversations/*.jsonl
                    memory/*.md
                    manifest.json
    ↓ 挂载该用户的目录
Sandbox：按时间/主题选文件 → 搜索并读取上下文
    ↓
模型回答 + 原会话/消息来源
```

以下为对应官方接口的方案示意，未运行验证：

```ts
await sandbox.mountBucket("PERSONAL_FILES", "/data/personal", {
  prefix: `/users/${authenticatedUserId}/`,
  readOnly: true,
});
```

用户标识由服务端已认证身份确定，不能由模型指定。挂载路径筛选不能替代服务端权限校验；部署验证需覆盖越权目录访问。每个沙箱的文件系统在其 sessions 间共享，因此不能用同一沙箱的不同 session 隔离不同用户。[存储 API](https://developers.cloudflare.com/sandbox/api/storage/)、[挂载指南](https://developers.cloudflare.com/sandbox/guides/mount-buckets/)

检索可以执行 grep/Python，但先用 manifest 与日期/主题候选缩小范围，再将热数据复制到本地临时目录。SQLite 索引作为可重建缓存在本地盘运行，不把对象存储挂载路径直接当作数据库盘。以上是本项目设计建议，需用中文型号、短词、跨日事件等合成记录验收。

原始历史只读，自动记忆更新返回结构化变更，Worker 验证来源和 revision 后写入。旧会话删除或明确“忘记”时更新清单与缓存失效信息，避免旧沙箱继续提供已排除的数据。

## 国内等待时间：必须看完整调用路径

Cloudflare Containers 最新 placement 文档已经包括 **APAC**；早期只列欧美的更新记录已不能代表现状。但 APAC 是大区域，不代表中国大陆部署，也不能据此保证具体网络延迟。[Cloudflare placement](https://developers.cloudflare.com/containers/concepts/placement/)

Daytona 当前共享区域列出 US/EU，自有或专属区域另行配置。[Daytona regions](https://www.daytona.io/docs/en/regions/)

Modal 可选择亚太计算区域。沙箱 exec 等数据请求直达所选区域的容器，创建/终止等控制请求仍经过美国控制面；不能套用其普通 Functions 的默认路由来描述全部沙箱请求。[Modal region selection](https://modal.com/docs/guide/region-selection)、[数据路径](https://modal.com/docs/guide/data-residency)

AgentRun 地域文档列出杭州、上海、北京、深圳和新加坡；FC 云沙箱是其升级路线，具体沙箱功能与地域组合仍需在 FC 当前配置中验证，不能直接把 AgentRun 地域表当作 FC 所有特性的保证。[AgentRun 地域](https://help.aliyun.com/zh/agentrun/open-service-area)

如果客户端仍先访问境外 Worker，再由 Worker 调用国内沙箱，请求仍有跨境段。国内路线要同时评估入口服务、沙箱、文件存储与模型的放置位置。Cloudflare 路线可以减少对另一个沙箱提供商的调用，但 Worker、Containers、R2 是不同服务，不能宣称没有网络跳转。

## 下一步验证范围与成本口径

建议只做两组相同负载的验证：Cloudflare Sandbox + R2（APAC），以及阿里云沙箱 + 同地域文件存储。先使用合成记录；保持相同问题、模型和检索规则，对比冷启动、已运行沙箱、首次/增量同步、中文检索、结果回传、首字和总耗时的 p50/p95，并检查删除后重查、并发更新与沙箱销毁后文件恢复。

挂载不是完整记忆产品：各路线都还需要 Potato 的对话同步、事件日期解析、来源标识、模型工具往返和记忆管理。先把这些接口与执行提供商分离，后续按实测结果选择。

不能只比较单个“沙箱每秒价格”。统计同一负载的计算、空闲保温、持久存储、对象读写、网络传输和入口服务成本。Cloudflare 官方说明 Sandbox 还涉及 Workers、Durable Objects 等计费；R2 存储也需计入。本轮未测用量，不给出哪个方案最便宜的结论。[Cloudflare 计费组成](https://developers.cloudflare.com/sandbox/platform/pricing/)

结论：Cloudflare 的文件挂载能力足以承载用户提出的个人对话空间，建议作为现有架构下的首要验证对象；如果国内低延迟权重更高，则让阿里云路线与它做同负载比较。Daytona、Modal 是功能成立的备选，暂不因提供商数量增加而扩大实施范围。
