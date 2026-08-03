# 任务:把当前工作区整理成一组干净的 git 提交(不要 push)

当前仓库 ~644 处变更:新前端 app/(untracked)、旧 console 前端大量删除、workflows 与脚本修改、分发管线新文件。把它们整理成**逻辑清晰的提交序列**。只 commit,**严禁 push**;严禁 `git add -f`;严禁改 .gitignore 之外的忽略规则。

## 提交分组建议(可按实际微调,但要逻辑自洽)

1. `feat(app): Codex-style desktop frontend rewrite` — app/ 全部新文件(src、配置、docs/)
2. `chore(console): retire legacy console frontend` — console/src 等删除项与相关修改
3. `feat(release): self-hosted update distribution pipeline` — scripts/pack-tauri/release_desktop.sh、RELEASE.md、tauri.conf.json 的 updater 变更
4. `ci: workflow updates` — .github/ 全部修改
5. 其余修改按内容归组(backend/config、README、.gitignore 等),不要一锅炖

## 安全红线(每个提交前自查)

- `git status` 确认没有把 node_modules/dist/.venv 等忽略目录带进来
- 新增文件里不得包含任何密钥/token:提交前对暂存内容跑
  `git diff --cached | grep -iE "api[_-]?key|secret|token|sk-[a-zA-Z0-9]|cfat_|PRIVATE KEY"`,
  命中的逐个人工判断(公钥、i18n 文案里的"API key"字样等误报可放行,真凭据必须剔除并报告)
- 单文件 >5MB 的新增要在报告里列出并说明(截图类历史上是允许的)

## 提交信息格式

沿用仓库现有风格(conventional commits,中文正文可选),每条末尾加:

```
Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

## 完成后输出

`git log --oneline qwenpaw/main..HEAD`(或自 db501215 起)的完整列表 + 每个提交的文件数/增删行摘要 + 安全自查结论 + 未纳入提交的剩余文件清单及理由。
