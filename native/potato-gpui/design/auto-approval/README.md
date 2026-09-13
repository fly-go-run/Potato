# 原生模型自动审批验收

实现依据与边界见 [架构记录](../../../../docs/architecture/approval-redesign.md)。本目录的 Python 服务是固定场景的假模型，只用于验收，不能作为真实审批器。

## 使用

新安装默认启用**自动审批**；权限菜单可切换自动或手动审批，明确保存的手动选择会保留。默认跟随当前对话模型；设置 → 安全可指定独立的审批服务商和模型 ID。仅需额外授权时调用审批模型；批准后继续执行，拒绝返回具体原因，缺少授权或审批故障才显示人工卡。

目录规则优先复用。模型批准不会自动保存永久权限。每次确认 / 禁止需确认的操作仍保留各自语义，输入框标签显示实际生效方式。

同一任务中，相同参数的低风险 `read_file` 批准可复用 10 分钟。只缓存 1 MiB 以内的普通文件；文件内容、文件身份、用户授权、权限设置或审批连接改变时重新审查。不同文件、不同行号、shell、写入和网络操作不复用这个批准。重启后模型批准缓存失效，持久审批记录仍可查看。打开右侧栏并展开「自动审批」记录，可看到「复用批准」及「本次未调用审批模型」，也可点击「清除本任务的审批复用」。

未命中决定缓存时，审批器保留有限的独立审查对话，用户授权和配置未变就追加当前动作；并发请求从最后提交的历史分叉。服务商可能利用稳定前缀缓存，但这类「延续审批上下文」仍然调用模型，不等于免费或跳过审查。原始用户消息和真实提问卡回答是授权依据；最近助手/工具文字及历史审批仅作参考。界面记录会显示两类证据的条数。

## 回归验证

```sh
cargo +1.96.1 test --locked --manifest-path native/potato-core/Cargo.toml
cargo +1.96.1 test --locked --manifest-path native/potato-gpui/Cargo.toml -- --test-threads=1
```

2026-09-09 最终回归：核心 212 通过、1 忽略；GPUI 65 通过。新增用例覆盖两个模型协议、空工具列表、读取批准、shell 拒绝、ask_user、无效 JSON/证据、违规工具输出、超时/超大响应、故障重试、取消与权限/目标变化、原始用户证据隔离、配置恢复、规则跳过以及三次拒绝后终止主循环。

原生客户端实测了权限菜单启用自动审批、审批进行状态、批准后直接读取外部文件、拒绝结果、故障卡的「允许本次」后继续完成，以及在设置中保存独立服务商与审批模型。验收过程中窗口截图出现刷新滞后，通过缩放窗口重绘后核对界面；核心执行结果另以持久审计和假服务调用日志核对。

## 本机假模型与 GPUI 闭环

每个场景使用一个新任务。假服务只绑定 127.0.0.1，在独立临时目录里创建三个测试文件，不访问真实模型。

```sh
python3 native/potato-gpui/design/auto-approval/mock_provider.py /tmp/potato-auto-approval-review
```

记录输出的本机 URL，然后初始化隔离数据：

```sh
cargo +1.96.1 run --locked --manifest-path native/potato-gpui/Cargo.toml --example approval_review -- /tmp/potato-auto-approval-review/data http://127.0.0.1:输出端口
cargo +1.96.1 build --locked --manifest-path native/potato-gpui/Cargo.toml
POTATO_NATIVE_DATA_DIR=/tmp/potato-auto-approval-review/data native/potato-gpui/target/debug/potato-gpui
```

在隔离客户端启用模型自动审批。每次新建任务后，分别输入普通读取请求、包含「拒绝」的测试请求、包含「故障」的测试请求。服务根据场景调用 small.py / deny.py / failure.py；正常场景自动读出两行内容，拒绝场景不读取，故障场景产生明确的人工回退卡。故障回退可选择允许本次或拒绝。

审计可使用以下只读命令查看：

```sh
native/potato-gpui/target/debug/examples/approval_review /tmp/potato-auto-approval-review/data inspect
```

`requests.jsonl` 只保存假模型调用种类、工具数量和模型名；正常流程为 main-tool-request（有工具）→ review-allow（零工具）→ main-after-tool（有工具）。审计可核对批准来源、证据 ID、模型、耗时与用量。

第三阶段复用场景使用新的隔离目录，例如 `/tmp/potato-auto-approval-reuse`。新建任务后输入「复用测试：请连续两次读取 small.py，再读取 second.py」。假模型会顺序提出三个读取动作；预期审计为 `model` → `model_allow_cache` → `model`，服务端只有两次 reviewer 请求。第三个动作应显示延续审批上下文，两个 reviewer 请求的 `base_digest` 相同，第二个请求的 `prior_assessments` 大于零。日志还记录消息数量和稳定前缀摘要，不保存用户密钥或完整上下文。此场景必须在一次主助手回合内完成；再发一条用户消息会改变授权证据，按设计要求重新审批。

此流程没有修改 `~/.potato` 的日常数据或配置。不要让多个原生客户端同时打开同一个数据目录。
