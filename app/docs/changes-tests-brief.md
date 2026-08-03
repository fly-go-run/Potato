# 任务:为 app/src/lib/fileChanges.ts 编写 vitest 单元测试

你是严谨的测试工程师。只做两件事:

1. 新建 `app/src/lib/fileChanges.test.ts`,为 `app/src/lib/fileChanges.ts` 写覆盖边界的单元测试。
2. 顺带审查该模块逻辑;如发现疑似 bug,**不要改实现**,在最终输出里列出「疑似问题」清单(文件:行号 + 触发场景)。

## 背景

`collectFileChanges(messages)` 从会话消息流聚合文件改动(write_file/edit_file/append_file),按 file_path 合并,统计 ±行数。消息/工具对的构造方式参考现有测试 `app/src/components/chat/ConversationSidePanel.test.ts` 里的 `toolMessage` helper(可以照抄或抽取类似 helper)。工具对状态判定逻辑在 `app/src/components/chat/ToolCard.tsx`(`toolPairStatus`/`buildToolPair`)。

消息结构要点:
- 工具调用:`type:"function_call"`(或 plugin_call/mcp_tool_call),content 里有 `{type:"data", data:{call_id, name, arguments:"<JSON字符串>"}}`
- 工具输出:`type:"function_call_output"`,data 里有 `{call_id, name, output, state}`
- edit_file 参数:`{file_path, old_text, new_text}`;write_file/append_file:`{file_path, content}`

## 必测用例

1. edit_file 成功对:断言 additions/deletions 的具体数值(与 `lineDiff` 一致)、before/after 保留原文。
2. write_file:additions=content 行数,deletions=0,before="";空 content → additions=0。
3. append_file:计新增行。
4. 同文件多次操作合并:additions/deletions 累加、edits 按时间序、lastMessageId 是最后一次调用的消息 id、kind 合并规则(edit 之后又 write → "write_file";append+edit → "edit_file")。
5. 多文件:输出按「首次触碰」顺序。
6. 不计入的情形,逐一覆盖:
   - output.state 为 "error" 的失败调用
   - 无 output 的运行中调用(call.status 无论如何都不算)
   - output.status 为 "cancelled"
   - read_file / send_file_to_user 等非改动工具
   - arguments 非法 JSON、缺 file_path、edit_file 的 old_text+new_text 均为空字符串
7. `totalChangeStats`:files/additions/deletions 汇总;空数组 → 全 0。

## 验收

- `cd app && npx vitest run` 全部通过(包括既有测试)。
- `cd app && npx tsc --noEmit` 干净。
- 只新增测试文件,不改任何现有文件。

完成后输出:测试文件路径、用例数、vitest 结果摘要、疑似问题清单(如无写"无")。
