# 回复体验与手机云端计算

2026-09-12。沿用第 1 张浅色方案，原生 SwiftUI；模型继续使用桌面土豆的 DeepSeek。用户确认沙箱用途为运行 Python、分析文件、生成图表和文档。

## 当前界面的审查

本轮实际截图：[改动前回复](../../../native/potato-ios/qa/reply-audit/01-current-reply.png)。图片只显示文件名与大小，需要逐个打开才能辨认；回复按钮没有文字选择/单条分享入口，朗读按钮缺少播放状态。这是对当前 Potato 的截图审查，竞品部分来自官方资料研究，没有冒充 ChatGPT/Claude 真机操作审查。

落地交互：

1. 回复完成后显示复制、朗读、末条重试和更多菜单。复制有勾选反馈；朗读可停止；更多包含选择文字、单条分享、存为文稿。空回复禁用内容操作。
2. 首字前显示“正在准备回复”，输出中显示“正在回复”，停止后保留已收到内容。拖动阅读时暂停自动跟随，点回到底部恢复。
3. 待发图片为缩略图横排、显示附件数量与移除按钮；系统照片选择按用户点击顺序排列。消息内单图大卡、多图双列网格，每条最多4个附件。
4. 点击第几张就从该张打开原生 Quick Look；可前后翻页、查看数量、分享当前附件。四图及其顺序重启后保留。

## 官方竞品资料与采用范围

- [ChatGPT iOS FAQ](https://help.openai.com/en/articles/7885016-chatgpt-ios-app-faq)：确认系统设置、触感等原生行为。当前FAQ未完整说明每个回复按钮位置，因此不据此宣称像素或按钮排列完全一致。
- [Apple 的 ChatGPT 应用介绍](https://apps.apple.com/gd/iphone/story/id1689259447)：介绍过通过回复操作重新生成。此页面不是当前版本完整交互规范。
- [Claude 文件上传](https://support.claude.com/en/articles/8241126-upload-files-to-claude)：文件/照片通过输入区添加，支持多文件；本项目保留适合当前 Worker 的4附件限制，不照搬供应商配额。
- [Claude 文件创建](https://support.claude.com/en/articles/12111783-create-and-edit-files-with-claude)：移动端支持执行代码和生成文件，下载可进入系统预览或其他应用。采用“回复内计算结果与文件卡片 → 系统预览/分享”的路径。

## 第三方沙箱比较

| 方案 | 官方当前费用与试用 | 对本项目的判断 |
| --- | --- | --- |
| [E2B](https://e2b.dev/pricing) | Hobby无基础订阅费，100美元一次性额度；运行另按秒计费，默认2 vCPU / 4 GiB约0.1656美元/运行小时；Hobby会话最长1小时 | 首选试接，Code Interpreter直接提供Python、标准输出与图表结果 |
| [Daytona](https://www.daytona.io/pricing) | 页面提供200美元计算试用；CPU 0.0504美元/vCPU小时，内存0.0162美元/GiB小时，存储另计 | 更广泛开发环境的备选；首轮不同时引入两套服务 |
| [Cloudflare Sandbox](https://developers.cloudflare.com/sandbox/platform/pricing/) | 按Containers资源计费，还涉及Workers、Durable Objects等 | 与现有平台接近，但不能把Worker免费请求额度理解为免费Python机器 |

以上是2026-09-12官方页面信息；试用额度不是每月免费额度。E2B默认配置运行1分钟约0.00276美元，仅作资源费示例，不包括模型与Worker费用。真实账单以账号为准。

## 接入结构与试运行边界

`iPhone → 已认证的 Worker /v1/sandbox/run → E2B Code Interpreter → 结果回存 iPhone`。

原生回复里的Python代码块提供“运行 Python”入口。面板可查看代码、勾选输入文件，显示文件在沙箱里的名称；点击运行才发送。当前为显式运行代码的试接版本，尚未实现模型自动规划、调用工具、修错重跑的完整Agent循环。

每次运行创建独立沙箱，禁止容器主动联网，不注入DeepSeek或Cloudflare凭据。输入最多4个文件、合计约2 MB，代码最多32000字符；Python执行60秒，沙箱TTL120秒，完成/失败后销毁，取消时尝试销毁。单独限速3次/分钟，按Cloudflare地点生效，不是严格全局费用上限。

图片结果返回PNG/JPEG，文档从 `/home/user/output` 取回；接受PDF、CSV、Markdown、TXT、DOCX、XLSX等有限格式。单文件最多2 MB，总产物base64最多4 MB，最多8个。产物与执行日志保存在对应回复，回复旧版本仍保留其产物引用；原生预览不执行HTML或任意网页内容。

E2B凭据与历史模板现已验证，用户明确授权后保存为Worker的 `E2B_API_KEY` Secret。iPhone经Worker真实执行、图表和文档回存、PDF预览及重启恢复通过。未配置该Secret的其他环境返回503，不返回模拟的“计算成功”。无需VPS。

SDK依据：[E2B Quickstart](https://docs.e2b.dev/quickstart)、[官方SDK](https://github.com/e2b-dev/code-interpreter)。实现还对安装的官方类型与回调行为进行了检查。本次验证的是已有自定义Office/PDF模板，不把它等同于所有E2B默认模板的依赖。

## 找回的历史接入

用户提示另一个项目曾接过沙箱后，定位到 `/Users/liuxu/lifeProjects/chat-web-dev`：

- `server/internal/sandbox/e2b.go`：已经实现E2B原生HTTP适配、同请求内复用沙箱、输入文件去重、结果下载与销毁。
- `e2b-template/template.ts`、`build.ts`、`probe.ts`：自定义 `chat-web-office-pdf` 模板，预装LibreOffice、Poppler、CJK字体和DOCX/XLSX/PDF所需Python包；基于 `code-interpreter-v1`。
- `docs/SANDBOX-SKILLS.md`：文档、表格、PDF技能包注入及产物/视觉检查文件的区分。
- 历史任务“提取远程沙箱接入配置”（019f8b30-90d0-7421-8982-d74c89313647）记录后来又改为本地Mac的sandboxd/Colima，经旧VPS反向隧道接入。它与托管E2B是两套不同provider。
- `docs/DOCKER-PERSISTENT-SANDBOX.md` 与 `docs/SANDBOXD.md` 记录E2B配置保留在旧服务器 `/etc/chat-web/chat-web.env` 作为回退。历史记录不是当前在线状态证明，需要只读核实。

本次优先复用已有E2B模板和能力设计，手机API继续部署在Cloudflare。不会把旧VPS/Mac隧道当成本次部署架构，也未修改旧项目代码或服务。

初次搜索时本机缺少历史SSH配置，旧服务器连接被关闭，未能读取远端凭据。之后用户提供完整E2B API Key：官方认证200，已有模板启动成功；自动审批要求指定账户持久保存的明确授权，用户确认后已完成Worker Secret保存。没有从旧项目读取或迁移密钥，也未改变旧服务。

验证包括：iPhone完整37项回归、图片预算调整后29项关联回归、四图真实问答、E2B真实执行与文件预览；图表去重后29项单元测试。同一SDK上传三行合成CSV，正确合计60并生成图表、PDF、Word和Excel。手机系统文件提供器导入与勾选分析组合尚未完整验收。截图及各阶段边界见 [原生验收记录](../../../native/potato-ios/design-qa.md)。
