# 原生 Shell 沙箱与自动恢复

日期：2026-09-10。仅覆盖 `potato-core` 与 `potato-gpui`。

## 已接入的行为

macOS 的 Shell 命令及子进程通过 Seatbelt 执行。`workspace-write` 允许项目普通文件读写，`read-only` 仅允许项目读取；两者均允许独立临时目录写入，默认禁止命令联网。必要系统库、工具链目录可读，秘密文件默认不可读，项目策略文件不可写。项目之外的用户文件和 Potato 私有数据默认不可读写。

Windows 已接入 LPAC 后端，参考 Codex 的权限准备、句柄白名单、Job Object 与恢复编排。当前 Windows Shell 的有效权限为项目只读、任务临时目录/profile 可写；要求 Shell 写项目时经过单次宿主执行审批，原生文件工具继续按配置编辑。实际能力在审批上下文与 GPUI 中明确呈现。详细源码依据、平台边界与验收状态见 [Windows 后端说明](../../native/potato-core/windows-sandbox/README.md)。

一次命令是一个 job，各次尝试独立归档。疑似沙箱拒绝触发诊断，再申请扩大权限；`AUTO + model` 使用现有独立审批器，批准后在同一个 job 自动重试。前台等待最终结果，后台 job 自行完成简单命令的恢复。第一次沙箱失败不会直接结束逻辑命令。

- 联网失败优先申请 `network_access=true`，保留文件沙箱。
- 其他限制可申请 `sandbox_permissions=require_escalated`，审查后以宿主账号权限执行一次；不调用 sudo，不修改会话默认模式。
- 每次 Shell 都走动作审批；`AUTO` 不意味着所有项目命令免审。`STRICT` 手动确认，`NEVER` 禁止需要审批的动作。
- 自动审批拒绝后不执行更宽权限；审批失败或缺少授权时由现有人工卡片处理，批准后自动继续。Shell 不保存会话级批准；文件读取授权仍沿用原有规则。
- 复合命令可能已产生部分副作用，返回 `needs_diagnosis` 和已有输出，提示主代理检查后继续剩余动作。错误关键词只触发审查，不能产生授权；普通非零退出不自动提权。
- 一个 job 最多两次执行；每次真实用户请求最多产生 3 次恢复计划，预算跨工具调用和重启保留，系统续跑通知不会重置预算。取消、授权撤销、新用户消息、策略版本变化和项目目录身份变化会阻止旧任务继续执行。重启保留输出，标记中断，不重放命令。
- 后台复合命令需要诊断时，调度器在当前回合结束后自动唤醒助手，要求读取输出、检查部分结果并继续剩余工作。助手已读取终态结果时不再额外唤醒；取消、新指令、撤销和归档会使旧通知失效。通知只保存在内存，重启不会自动续跑。已有用户消息优先。

GPUI 安全设置显示实际后端可用性；右侧审批记录显示恢复阶段、每次文件/网络权限及退出码。`job_output` 的可选 `attempt` 参数可读取首次失败的完整归档。

## 代码位置

| 文件 | 职责 |
| --- | --- |
| `native/potato-core/src/sandbox/mod.rs` | 实际可用性探针、执行计划、环境过滤、Seatbelt 策略 |
| `native/potato-core/windows-sandbox/` | Windows LPAC、临时身份与 ACL、原生进程与整个 Job 清理 |
| `native/potato-core/src/execution_recovery.rs` | 前后台共用的诊断、审批、一次重试和失效检查 |
| `native/potato-core/src/shell_followup.rs` | 后台诊断续跑、真实用户授权边界与跨调用恢复预算 |
| `native/potato-core/src/processes.rs` | 受限进程启动、描述符清理、输出、取消与超时 |
| `native/potato-core/src/jobs.rs` | 多次尝试的状态、归档和重启恢复 |
| `native/potato-core/src/approval.rs`、`reviewer.rs` | 可信执行上下文、人工/模型审批、缓存隔离 |
| `native/potato-gpui/src/interactions.rs`、`settings.rs` | 恢复记录与实际能力展示 |

复用 Codex 的两份 Seatbelt 基础策略，固定提交 `2cbbf0c9b542a36a1c3284b5e804917635b6f666`，保留 `sandbox/LICENSE.codex`。Potato 增加受限读取、独立 HOME/缓存/临时目录、凭据环境过滤、保护路径和已有硬链接检查。macOS 26 的加载器需要根目录节点读取权限；这不授予整个根目录子树读取。Seatbelt 正则不支持 PCRE `(?i)`，保护规则显式展开 ASCII 大小写。

## 第一版边界

- Linux 尚无 OS 后端。Windows/macOS 探针不可用或平台未实现时，命令必须通过明确的无沙箱动作审批；不会静默裸跑。已有目录拒绝规则存在时，不提供无法执行这些拒绝的无沙箱回退。Windows 当前尚无项目可写的 Shell 沙箱，也没有 Codex 的独立账号/管理员服务路线。
- 只实现关闭网络与获准后的出站 IP 联网，尚无域名代理、端口细分和按文件路径追加权限。联网批准的范围会明确呈现给 reviewer。
- 环境不继承真实 HOME 配置、密钥、SSH agent、代理或动态库注入变量。需要这些配置的开发工具可能需要诊断和另行授权。
- 目录拒绝规则保守映射为 Shell 的读写拒绝；已有文件工具的目录允许规则不会自动扩大 Shell 权限。
- 文件 metadata 和必要系统目录是可读的，不能将此实现描述为隐藏所有宿主信息的容器。已有硬链接检查最多 100000 条目；超限需要较小项目或另行审查。它不是对抗其他宿主进程并发修改文件系统的完整防线。
- 沙箱拒绝目前依赖退出结果中的启发式信号，尚无完整 OS 拒绝事件采集。单一脚本也可能部分执行，其重放安全性仍由独立 reviewer 判断。
- 恢复预算按当前真实用户请求累计：已进入恢复后，助手另行申请联网/宿主执行也计入预算。达到上限后可继续现有权限内工作，但不继续自动申请扩大权限；明确拒绝另外受原有拒绝缓存和熔断约束。
- 后台诊断沿用桌面端的进程内调度器；应用关闭时不执行。通知在显示历史中保留为系统来源的助手说明，审批器只使用原始用户消息/真实回答作为授权。模型传输上下文中的运行时通知明确标注不构成用户消息或新授权。
- macOS 进程组支持取消和超时，但不能据此声称能清理主动脱离进程组的所有后代。子进程继承 Seatbelt。Windows 使用禁止脱离的 Job Object，Shell 退出也会清理后代。MCP、电脑操作、模型请求、联网搜索和内部 Git 等独立工具不在此次 Shell 沙箱覆盖内。

## 验证

普通回归：

```sh
cargo +1.96.1 test --locked --manifest-path native/potato-core/Cargo.toml
cargo +1.96.1 test --locked --manifest-path native/potato-gpui/Cargo.toml -- --test-threads=1
```

实际 Seatbelt 用例需要在 macOS 宿主环境执行，不能嵌套在调用代理的沙箱里。测试仅访问临时文件和本地模拟 HTTP/审批服务：

```sh
cargo +1.96.1 test --locked --manifest-path native/potato-core/Cargo.toml --lib sandbox -- --include-ignored
```

覆盖项目读写、只读模式、项目外读取、秘密文件、符号链接、已有硬链接、保护目录、子 Shell 权限继承、禁网与保留文件隔离的联网，以及前后台自动审批恢复、单次沙箱外重试、审批拒绝、取消/撤销和部分副作用诊断。模拟审批器验证编排，不代表真实模型的审批准确率。尚未完成 IPv6、UDP、Unix socket 和敌对脱离进程组后代的全面验收。

Windows 接入后的 macOS 核心回归共 228 项通过，排除两个需要真实模型配置/桌面驱动的可选探针。新增后台续跑、取消/撤销/新指令/已读取/重启去重及跨重启预算用例。GPUI 65 项测试通过，两个项目的全目标 Clippy 均通过。先前滑块测试失败源于异步回调跨线程唤醒确定性测试调度器；原生视图测试改用真实后端的同步响应夹具，独立适配器测试继续覆盖异步路径。原有静态告警也已修正。

实际应用启动验收发现 Rosetta 子 Shell 需要额外读取 `/Library/Apple/usr/libexec/oah/libRosettaRuntime`，现已精确开放该系统库并增加 Intel 子 Shell 用例。修复后 11 项真实沙箱/恢复相关测试通过；GPUI 完成了真实进程、模拟模型的闭环：第一次复合命令禁网失败，自动唤醒助手只补做下载，第二次保留文件隔离并成功，`marker` 始终只写入一次，最终结果自动载入会话。见[截图、审计与复现方法](../../native/potato-gpui/design/sandbox/README.md)。模拟模型验收不代表真实模型审批准确率。

Windows 后端与完整核心已通过 GNU Windows 目标的交叉编译及全目标 Clippy。当前无 Windows 实机，新增真实 LPAC/ACL/进程测试已接入 Windows CI，但本轮未触发远程运行，不能把交叉检查或 macOS 回归作为 Windows 实机通过的证据。
