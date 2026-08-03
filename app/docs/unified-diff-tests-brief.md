# 任务:为 app/src/lib/unifiedDiff.ts 编写 vitest 单元测试

你是严谨的测试工程师。只做两件事:

1. 新建 `app/src/lib/unifiedDiff.test.ts`,为 `parseUnifiedDiff` 和 `matchRepoRelativePath` 写覆盖边界的单测。
2. 审查模块逻辑;疑似 bug **不要改实现**,在最终输出列「疑似问题」(文件:行号 + 触发场景)。

## 关键要求:测试样例必须用真实 git 输出

不要手写想象中的 diff。在本仓库(或临时目录 `mktemp -d` 里 git init 的仓库)实际执行 git 命令生成样例,把输出原样(含结尾换行)嵌入测试字符串。至少覆盖:

1. 单文件多 hunk 修改:断言每个 hunk 的 oldStart/oldCount/newStart/newCount、逐行 kind 与 oldLine/newLine 行号推进正确、文件级 additions/deletions。
2. 新增文件(`git diff --no-index -- /dev/null <file>`,即后端 untracked 路径用的形式):isNew、oldPath===""、行号从 1 开始。
3. 删除文件:isDeleted、newPath===""。
4. 重命名(`git mv` 后 `git diff --staged -M`):isRename、oldPath/newPath。
5. 二进制文件(如一个小 png 或 `printf '\x00\x01'`):isBinary、hunks 为空。
6. CRLF 内容的 diff(行尾 \r 被剥掉)。
7. 文件末尾无换行(产生 `\ No newline at end of file` 标记):标记行不产生 UnifiedDiffLine、不影响行号。
8. 空输入 "" → []。多文件串联 diff → 每个文件独立正确。
9. hunk 头带函数上下文(`@@ -1,3 +1,4 @@ function foo()`)→ section 字段。
10. 含中文/空格路径(git `core.quotepath=false` 输出)。

`matchRepoRelativePath`:
- 绝对路径后缀命中;多候选取段数最多者(如候选 ["x.ts","b/x.ts"],绝对路径 .../a/b/x.ts → "b/x.ts");不命中 → null;反斜杠路径归一;"./" 前缀候选可命中。

## 验收

- `cd app && npx vitest run` 全过;`npx tsc --noEmit` 干净。
- 只新增该测试文件,不改任何现有文件。

完成后输出:用例数、vitest 摘要、疑似问题清单(如无写"无")。
