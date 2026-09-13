# 桌面搜索与语音迁移

2026-09-12。用户要求只迁移桌面土豆的Exa搜索，并沿用豆包语音API；明确要求由模型自动搜索，不提供联网开关。

## Exa自动搜索

桌面实现依据 `native/potato-core/src/search.rs`。凭据来自可信应用配置 `~/.potato/.env`，不读取当前项目.env。Worker使用同一Exa `/search` 接口：每次5条、每条最多2000字符。仅返回安全HTTPS来源并去重，不添加第二个搜索引擎。

Worker向现有DeepSeek模型提供唯一的 `web_search` 工具，由模型决定是否调用。兼容模型在同一轮发起多个查询，按顺序执行；每条回复总共最多4次搜索，超出预算的调用返回明确工具结果，然后要求模型回答。工具参数的流式片段会拼接；保留DeepSeek继续工具循环需要的reasoning_content；中间轮的DONE不会提前结束客户端流。检索结果作为不可信数据提供，要求引用实际来源网址，搜索失败不伪造来源或转用其他引擎。

iPhone接收搜索开始/完成/失败事件，展示进度、查询与来源面板，查询结果随回复版本持久化。停止生成及进程重启会结束旧搜索等待状态。连接探测的16-token请求不启用工具。

官方接口：[Exa Search](https://exa.ai/docs/reference/search)。搜索限速10次/分钟，沿用单设备鉴权；属于Cloudflare地点级限速，不是严格全局费用控制。

## 豆包语音输入

桌面实现依据 `native/potato-core/src/voice.rs`。使用原生数据库中已启用的豆包配置与相同资源 `volc.seedasr.sauc.duration`，同一 `bigmodel_async` WebSocket端点。密钥仅在服务端；iPhone继续使用现有设备令牌。

`iPhone AVAudioEngine → 16kHz/16-bit/mono PCM → Worker WebSocket → 豆包 → partial/final → 输入框`。

2026-09-12选定方向1后，点输入框麦克风直接申请权限并连接，原位实时转写。点“发送”排空重采样尾部、等待最终结果后提交；“修改”收尾并原位打开键盘。取消恢复原草稿，后台或切会话保留可用文字并清除发送意图；最长60秒，达到上限转为待编辑草稿。按进入时的光标位置替换本次听写片段，附件不丢失，不保存原始录音。详见 [语音设计与验收](voice-redesign/README.md)。回复朗读仍是系统TTS，本次迁移的是桌面已有的ASR能力。

Worker固定官方上游，gzip二进制协议与桌面一致；两端WebSocket在accept前显式设置arraybuffer，适配Cloudflare目前默认Blob的行为；partial中utterance definite不当作整个会话结束，只认协议LAST标志。输入单块64KB、总60秒，连接限时70秒，停止后等待8秒，限速3次/分钟。

官方运行时依据：[Cloudflare WebSocket二进制消息](https://developers.cloudflare.com/workers/runtime-apis/websockets/#binary-messages)。

## 凭据与验证

`scripts/desktop-services.mjs`只读可信应用配置、复制数据库及WAL读取一致视图，解密值仅在进程内。`migrate-desktop-services.mjs`向已审核的 `potato-iphone-api` 写入指定Secret，不改设备、模型或E2B密钥。自动审批要求明确外送授权，用户回复“允许”后才保存Exa和豆包凭据。

已通过：Worker18项测试、TypeScript检查；原生31项单元测试；Exa真实认证200；豆包官方接口对3.6秒合成语句正确返回“你好，土豆，帮我整理今天的工作计划。”。手机端最终联调与截图追加在原生 [design-qa.md](../../../native/potato-ios/design-qa.md)。合成音频验证不能代替真机麦克风质量、长录音与来电中断验收。
