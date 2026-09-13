# 过程轨迹 · 原生实现验收

日期：2026-09-08。实现范围：`potato-gpui` 与 `potato-core`。

## 对照与迭代

用户选定图见 [reference.jpg](reference.jpg)。图的底部是动效标注，不属于客户端界面。桌面回放逻辑窗口为 1280 × 780；原生截图由系统返回为 1261 × 768。对照图只裁取双方真实时间线区域，等比例归一化，不把系统窗口边框或图像密度误差当作视觉缺陷。

- [第一次并排对照](comparison-before.png)：P2，过程标题被组件默认布局居中；图标与文字间距偏紧。
- 修复：标题改为内部全宽左对齐容器；时间线缩进 24px，图标列 40px，列间距 12px；步骤标题 15px，摘要 14px。
- [修复后并排对照](comparison.png)：标题、节点与摘要层级清楚，连接线轻，完成节点静止，只有当前动作有旋转。
- 用户追加要求：移除过程标题及步骤行的悬浮灰底。已改用透明文字按钮，点击、键盘可达性与展开功能保留。[鼠标停留在收起标题上的原生截图](collapsed-hover.png)确认没有整行背景。[进行中标题悬浮（深色）](running-collapsed-hover.png)同样无背景。

## 视觉检查

| 检查面 | 结果与接受的差异 |
| --- | --- |
| 字体与层级 | 使用项目原生系统字体与中文回退。标题 15px，摘要 14px，计时 12px；当前动作比已完成动作更明确。相比设计图略紧凑，保持现有客户端阅读密度。 |
| 间距与布局 | 固定图标列、1px 连接线、统一内容起点。沿用现有 760px 正文宽度，而非扩展整个聊天布局。800px 窄窗口正常，详情不会盖住输入框。 |
| 颜色与状态 | 使用现有浅色/深色主题 token；完成、进行、失败分别使用弱化文本、正文色、错误色。未额外引入彩色状态卡。 |
| 图标与清晰度 | 使用组件库 Lucide 图标。正文界面未使用生成图切片或手绘替代图标。 |
| 文案与真实内容 | 动作名来自实际工具及文件名。思考内容仅呈现服务端提供的摘要，不生成虚构步骤名称。历史缺失计时不补造数字。 |

[浅色展开态](running.png) · [深色模式](dark.png) · [窄窗口](narrow.png) · [窄窗口工具详情](narrow-detail.png)

## 交互检查

- 原生实际点击：过程收起/展开、工具详情展开/收起、浅色/深色切换均可用。
- 回放正式回答后，时间线收为一行「执行了 3 个步骤 · 耗时」，正文保留，完成操作按钮出现；再次打开能查看原步骤。
- 旋转周期 1200ms，完成图标淡入 140ms，过程折叠 180ms。折叠测量自然高度，反向操作从当前高度续接；流式文本更新不重置折叠起点。
- 系统「减少动态效果」开启时旋转和高度动画均静止，展开/收起立即生效。
- 用户向上阅读或打开详情时，保留展开选择和预览节点，暂停自动跟随底部。相关状态与实际滚动偏移均有 GPUI 回归覆盖。
- 最近步骤预览保留所有仍运行的节点；并行执行集中显示一个动态指示。

## 数据与兼容

工具开始时记录 UTC 起点，实际完成瞬间记录单调时钟耗时。并行工具先完成时立即更新其状态，不等待前一个工具的有序结果返回。归档保存相同计时，重启后可还原。

Responses 的公开 commentary / final_answer 消息使用不同 UI 身份，避免过程说明在正式正文开始时移动或重复。模型明确提供 `final` / `final_answer` 阶段时，正式正文开始即收起；未提供阶段的模型在整轮结束时收起，避免提前把中途说明误判为最终回答。

未进行收费的真实模型调用，HTTP/SSE 验证使用本地协议回放。实际桌面验收覆盖 macOS；未声称完成 Windows 图形验收。

## 复现

```sh
cargo +1.96.1 build --locked --manifest-path native/potato-gpui/Cargo.toml
python3.12 native/potato-gpui/package.py --debug --output /tmp/potato-timeline-build
POTATO_NATIVE_DATA_DIR=/tmp/potato-timeline-data \
POTATO_GPUI_REVIEW_FIXTURE="$PWD/native/potato-gpui/design/process-timeline/running.json" \
'/tmp/potato-timeline-build/Potato GPUI Review.app/Contents/MacOS/potato-gpui'
```

`transition.json` 回放阶段切换和完成；`narrow.json` 覆盖窄窗口与减少动态效果。回放只在 debug 构建且明确指定隔离数据目录时启用。

final result: passed（视觉与交互范围如上；测试结果见实施说明）。
