# iPhone 流式输出流畅度调研

2026-09-16。依据用户对 TestFlight 回复「等一会一小块」的反馈，检查当前 iPhone 云端调用链及公开产品/组件文档。本轮仅调研，未修改运行代码、调用真实模型或部署服务。没有采集该次真机回复的网络与帧时间，不能据此量化模型、网络、存储各自的耗时。

## 当前实现中的确定性分块来源

`native/potato-ios/Sources/WorkspaceStore.swift:329`：已登录云端账号走 CloudReply job；手动连接才走原有 ChatService SSE。

`WorkspaceStore.swift:498–525`：GET 拉取事件页，同步应用，追上服务端进度后 sleep 750 ms，再发下一次 GET。下次数据到达的间隔还包含网络及处理耗时；积压页则直接继续读取。不是对手机保持连续的事件推送。

`CloudReply.swift:43–50`：每次请求新建 URLSession，使用 data(for:) 等待完整 JSON 页；`native/potato-worker/src/chat-job.ts:43–55` 每页最多 64 个事件。这是分页上限，不是必须积满 64 个才返回。

`WorkspaceStore.swift:549–586`：在 MainActor 中同步循环应用整页事件，最后保存完整 workspace。因此页面容易一次显示该页积累的文本，没有独立的显示缓冲/节奏层。

后台依然按 SSE 接收上游并记录事件（`chat-job.ts:105–127`），所以可在保留任务持久化的前提下增加前台实时订阅。

另有潜在渲染开销，尚需 Instruments 测量：每页保存时 JSON 编码并原子写整个 workspace（`LocalStorage.swift:19–22`）；MarkdownContent 在 body 中重新解析整个消息（`DocumentView.swift:102`）；文本变化触发追底（`WorkspaceView.swift:393`）。不能将这些风险直接等同于已经实测的瓶颈。

## 公开可验证的参考

| 产品 / 组件 | 文档公开的机制 | 对 Potato 的启示 |
| --- | --- | --- |
| LibreChat | SDK-backed 路径使用自适应平滑，默认目标 25 ms；首个文本 token 不延迟，积压时扩大块，保留工具和元数据顺序 | 首字立即显示，后续自适应追赶，避免固定慢速打字拖长回复 |
| Open WebUI | 全局流式批量阈值默认 1；增加批量可降低服务器负载，但官方明确说明 UI 会更跳跃 | 大块传输与流畅显示存在取舍，不能只优化请求次数 |
| Vercel AI SDK | smoothStream 缓冲并按词/行/自定义规则输出，默认延迟 10 ms；明确提示按空格切词不适合中文 | 中文按可见字符或短片段处理；10 ms 是其转换器默认值，不是各产品手机 UI 帧率 |
| assistant-ui | useSmooth 将已收到文本按前缀渐进显示，可配置追赶参数，尊重减少动态效果；resumable streams 保留服务端生产和断线补读 | 将真实数据、显示进度、恢复游标分开；后台继续生成和前台实时流式可以兼容 |
| Vercel Streamdown | 处理未闭合 Markdown，使用 memoized rendering 降低重复更新开销 | 稳定段落尽量复用，活跃尾段增量更新，减少格式跳变 |

来源（访问于 2026-09-16）：

- LibreChat：https://www.librechat.ai/docs/configuration/librechat_yaml/object_structure/shared_endpoint_settings#streamrate
- Open WebUI：https://docs.openwebui.com/reference/env-configuration/#chat_response_stream_delta_chunk_size
- Open WebUI 批量取舍：https://docs.openwebui.com/troubleshooting/performance/
- Vercel AI SDK：https://ai-sdk.dev/docs/reference/ai-sdk-core/smooth-stream
- assistant-ui 平滑：https://www.assistant-ui.com/docs/api-reference/utilities/miscellaneous#usesmooth
- assistant-ui 断线恢复：https://www.assistant-ui.com/docs/guides/resumable-streams
- Streamdown：https://github.com/vercel/streamdown

没有把 ChatGPT、Claude 等闭源客户端的具体缓冲大小或刷新频率当作已核实事实。

## 建议实施顺序

1. 在现有持久化 job 上增加可恢复 SSE 订阅：先按游标补齐历史，再连续发送新事件；断线只取消订阅，不停止生成。保持鉴权、停止、事件顺序、去重、终态以及先持久化后确认的语义。轮询保留为降级路径。无需为了单向文本推送强行引入 WebSocket。
2. iPhone 增加独立显示缓冲：首个可见文本尽快出现；有可显示内容时以约 25–40 ms 为首轮调参范围，小批呈现，积压时加速。按 Swift Character / 字素簇切片，兼容汉字和组合 emoji。文字已持久化不等于已显示；复制/分享、停止、完成、切会话、后台恢复需定义一致行为。减少动态效果时取消人工打字动画。
3. 将检查点写入与显示刷新分开，正文与恢复游标仍一致保存；缓存稳定 Markdown 段落，仅更新当前尾段。追底跟随显示进度，用户手动回看时保持位置。

以上时间值是 Potato 的待验证调参起点，不是已实现指标。平滑额外显示滞后可先以 100–200 ms 为目标，需结合中文阅读及网络抖动实测；上游停止提供新文本时，显示层无法消除真实等待。

## 验证方法

用不含个人内容的中文长文、列表、表格、代码与 emoji 测试，记录上游事件生成、手机接收、文本可见、最终完成四类时间。统计首字时间、接收间隔及可见更新间隔的 p50/p95、突发块大小、显示滞后、主线程长任务、滚动帧时间和持久化开销。

覆盖 50/200/1000 ms 不均匀上游输入、弱网与断网恢复、重复/缺号事件、工具事件交错、停止及重启、前后台切换、长历史、大字与减少动态效果。生产模型速度与人为平滑延迟分别报告。
