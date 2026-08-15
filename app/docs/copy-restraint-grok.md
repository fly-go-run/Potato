# Potato 文案克制轮

只读盘点，不改代码。判断标准只有一句：**这行字删掉后用户会不会做错事？不会就删。** 控件自己已经 labeled 清楚的，描述必删。不要因为担心用户看不懂而加小字。

读过：

- `app/src/views/SettingsView.tsx`（每个 `SettingRow` 的 `description`，含供应商详情 / 新建）
- `app/src/components/chat/Composer.tsx` 审批下拉
- `app/src/components/chat/TriggerPopover.tsx`
- `app/src/views/SkillsView.tsx`（`humanSkillName` + `skillDescription`）
- `app/src/lib/skillPresentation.ts`
- `app/src/lib/i18n.ts` 对应 zh/en
- `src/qwenpaw/agents/skills/*/SKILL.md` front matter

内置技能：`agents/skills/` 现有 **16** 个（各 zh/en 一套）。用户说的 17 来自旧界面计数（含已删的 `qa_source_index`，或文档里仍列、仓库已无目录的 `news`）。下表白名单按 16 个现役写，并附 `news` 兜底，避免旧工作区漏网。

---

## 1. 冗余小字盘点

### 1.1 设置 `SettingRow`

| 位置 | 标题（现状） | 现状 description | 处置 | 理由 | 新文案（缩短才填） |
|---|---|---|---|---|---|
| 模型 / 当前模型 | 当前模型 | 在输入框旁的模型选择器中切换模型。 | **删** | 右侧已显示模型名+供应商；换模型的入口不在这一行。 | — |
| 通用 / 主题 | 主题 | 主题会立即应用到整个工作台。 | **删** | 浅色 / 深色 / 跟随系统 三键自明。 | — |
| 通用 / 联网搜索 | 联网搜索方式 | 服务端 vs Tavily 的长段对比（见 `settings.webSearch.description`）。 | **删** | 选项自己已写「自动（有密钥就用服务端搜索）」「Tavily（免费）」。 | — |
| 同上，无密钥且档位是自动 | 同上 | `needsKey`：还没有可用于服务端搜索的密钥… | **缩短** | 删掉后用户不知道为什么没走服务端。这是阻塞原因，不是说明书。 | 见 §1.4 |
| 同上，无密钥且显式选了服务端 | 同上 | `needsKeyStrict`：已指定服务端搜索，但没有可用的密钥… | **缩短** | 选错会搜索失败，必须留后果。 | 见 §1.4 |
| 通用 / 搜索模型 | 搜索用的模型 | 由上面选中的供应商执行搜索的模型，必须是…DeepSeek 用 deepseek-v4-flash。 | **缩短** | placeholder 已是 `deepseek-v4-flash`；只留「选错会搜不了」那句。 | 见 §1.4 |
| 通用 / 上下文用量 | 显示上下文用量 | 在输入框旁常显本轮已用的上下文百分比。默认关闭。 | **删** | 开关标题已说清；默认关不是用户会做错的事。 | — |
| 通用 / 自定义主题 | 自定义主题 | 导入 JSON 主题文件(含 name、base、tokens 字段)… | **删** | 「下载模板」「导入主题」两键自明；格式错已有 `importInvalid`。 | — |
| 通用 / 语言 | 语言 | 选择界面显示语言。 | **删** | 中文 / English 两键自明。 | — |
| 通用 / 记住窗口 | 记住窗口大小与位置 | 下次打开 Potato 时恢复你上次使用的窗口尺寸和位置。 | **删** | 标题已经是完整句子。 | — |
| 通用 / 恢复窗口 | 恢复默认窗口 | 立即设为 1280×800 并在当前屏幕居中。 | **删** | 点下去不会做错事；具体像素不是决策信息。 | — |
| 通用 / 语音（已配密钥） | 语音输入（豆包） | 使用 .env 中的火山语音密钥，经后端调用豆包极速版 ASR… | **删** | 标题已点名豆包；开关打开就能用。实现细节不必写在行上。 | — |
| 通用 / 语音（缺密钥） | 同上 | 未检测到豆包语音密钥。请在项目 .env 中配置… | **缩短** | 开关是灰的。不说原因用户会以为坏了。 | 见 §1.4 |
| 通用 / 技能与插件 | 技能与插件 | 已启用 {n} / 共 {m} 个技能 · {p} 个插件 | **保留** | 这是状态数字，不是说明书。列表行不要再加描述。 | 不动 |
| 安全 / 沙箱 开 | 启用沙箱隔离 | 命令在沙箱中执行，工作区外的访问受限。 | **缩短** | 关掉等于裸跑本机，选错有安全后果。 | 见 §1.4 |
| 安全 / 沙箱 关 | 同上 | 命令直接以当前用户身份执行，无隔离。 | **缩短** | 同上。 | 见 §1.4 |
| 安全 / 沙箱未生效 | 同上 | `unsupported` / `notAdmin` | **缩短** | 界面显示已开、实际没生效，不说会误判。 | 见 §1.4 |
| 安全 / 默认审批 | 默认审批档位 | 在输入框下方的审批选择器中调整。 | **删** | 右侧只是只读当前档；教人去别处改，不是防错。 | — |
| 数据 / 附件上限 | 附件大小上限 | 单个上传附件允许的最大体积。 | **删** | 标题 +「25 MB」自明。 | — |
| 数据 / 导出 | 导出工作区 | 把工作区全部文件打包为 zip 下载。 | **删** | 按钮文案已是「导出工作区」。 | — |
| 关于 / 品牌 | Potato | 帮你把工作做完的桌面 AI 同事。 | **保留** | 产品身份，不是操作说明。 | 不动 |
| 关于 / 后端 | 后端状态 | 已加载 {n} 个 agent。 | **保留** | 状态数字。离线时这行本来就不渲染。 | 不动 |
| 供应商详情 / 名称行 | （供应商名） | `localReady` 或 base URL | **保留** | 状态 / 数据，不是教怎么点。 | 不动 |
| 供应商详情 / API key | API key | 已保存。输入新 key 可替换。 / 此供应商不要求 API key。 | **保留** | 状态：已存或选填。删了看不出当前有没有 key。 | 不动 |
| 供应商详情 / Base URL | Base URL | 此供应商的地址由系统管理。（仅 freeze） | **保留** | 解释为什么输入框是灰的。未冻结时本来就没这行。 | 不动 |
| 供应商详情+新建 / 协议 | 接口协议 | 端点用哪套接口。Responses 是 Codex 用的协议，DeepSeek 等也已支持。 | **缩短** | Chat Completions / Responses 两键对普通人不等价，选错连不上。 | 见 §1.4 |
| 供应商详情 / 测试 | 测试连接 | 连接成功 / 失败原文 | **保留** | 操作结果，不是说明书。空闲时本来就没这行。 | 不动 |
| 新建供应商 / 名称 | 名称 | 用于生成标识符，需以字母或数字开头。 | **删** | placeholder 已是「如 my-gateway」；不合法已有 `invalidName`。 | — |

组头小字（不是 `SettingRow`，但同属「每行底下跟一句」）：

| 位置 | 现状 | 处置 | 理由 |
|---|---|---|---|
| 服务商列表头 | 管理连接凭据与模型列表，点击任意服务商进入详情。 | **删** | 行可点、「添加自定义供应商」已在。 |
| 模型管理头 | 移除不需要的模型，或手动添加模型 ID。 | **删** | 「发现模型」+ 添加表单已在。 |

确认框（删供应商 / 删技能）的 description **不在本轮范围**：那是破坏操作的确认，不是行内说明书。

### 1.2 Composer 审批 hint

现状（`Composer.tsx` 261–285，每档两行：label + hint）：

| 档 | 现状 label | 现状 hint | 处置 |
|---|---|---|---|
| AUTO | 自动 / Auto | 常规操作自动放行，仅高风险操作会询问。 | **删 hint** |
| SMART | 智能 / Smart | 智能判断，重要或有影响的操作会先询问。 | **删 hint** |
| STRICT | 严格 / Strict | 每个工具操作都需要你确认。 | **删 hint** |
| OFF | 关闭 / Off | 全部自动执行，不再询问，请谨慎使用。 | **删 hint** |

「自动 / 智能 / 关闭」本身不够自明，hint 是在补 label 的课。按原则应把含义写进选项，而不是选项两个字再跟一行解释。菜单节头「审批档位」保留（缩短的触发器文案靠它找回语境）。

**新 label（hint 整段删掉，`item.hint` 和下面那行 `<span>` 一起拿掉）：**

```ts
// i18n.ts — 替换 label，删除 *Hint 四键
"composer.approval.auto": "自动放行",
"composer.approval.smart": "重要先问",
"composer.approval.strict": "每次确认",
"composer.approval.off": "从不询问",

"composer.approval.auto": "Auto-allow",
"composer.approval.smart": "Ask if risky",
"composer.approval.strict": "Always ask",
"composer.approval.off": "Never ask",
```

触发器 chip 跟同一套 label，不再出现孤词「关闭」。

删掉的 key：`composer.approval.autoHint` / `smartHint` / `strictHint` / `offHint`（zh+en）。

### 1.3 可抄 i18n 补丁（设置，仅缩短项）

删的 key 直接停传 `description`，键可留着以后扫，不必本轮删字典。

```ts
// zh
"settings.webSearch.needsKey": "没有服务端密钥，将退回 Tavily。",
"settings.webSearch.needsKeyStrict": "已指定服务端搜索但无密钥，搜索会失败。",
"settings.webSearch.modelHint": "须是该供应商支持联网的模型。",
"settings.voice.descriptionMissingKey": "未配置豆包密钥，开关不可用。",
"settings.sandbox.on": "工作区外访问受限。",
"settings.sandbox.off": "以当前用户直接执行，无隔离。",
"settings.sandbox.unsupported": "本平台不支持，已开但未生效。",
"settings.sandbox.notAdmin": "需以管理员重启后生效。",
"settings.create.protocolDescription": "Codex 用 Responses，其余多用 Chat Completions。",

// en
"settings.webSearch.needsKey": "No hosted-search key; falling back to Tavily.",
"settings.webSearch.needsKeyStrict": "Hosted search is pinned but has no key, so search will fail.",
"settings.webSearch.modelHint": "Must be a model this provider hosts with built-in search.",
"settings.voice.descriptionMissingKey": "Doubao key missing, so this switch is off.",
"settings.sandbox.on": "Access outside the workspace is restricted.",
"settings.sandbox.off": "Runs as you, with no isolation.",
"settings.sandbox.unsupported": "Not supported here; on, but inactive.",
"settings.sandbox.notAdmin": "Takes effect after an admin restart.",
"settings.create.protocolDescription": "Responses is for Codex; most others use Chat Completions.",
```

计数：SettingRow **删 14 / 缩短 9 / 保留 8**；组头删 2；审批 hint 删 4，label 改 4。

---

## 2. 技能人话化

### 2.1 三处现状（已经分叉）

| 表面 | 文件 | 现在给人看的名字 | 现在给人看的描述 |
|---|---|---|---|
| `/` 弹层 | `Composer.tsx` 234–236 → `TriggerPopover` | **raw** `browser_cdp`（`item.value`） | **SKILL.md 原文**（写给模型的英文/中文触发说明） |
| 设置页技能列表 | `SettingsView.tsx` 1360–1361 | **raw** `skill.name` | 无（不要加） |
| SkillsView 列表/详情/池/Hub | `SkillsView.tsx` `humanSkillName` + `skillDescription` | Title Case：`Browser Cdp` | r10 白名单中文；未命中则回落 SKILL.md |

`skillPresentation.ts` 只有中文 description，**没有显示名，也不看语言**——英文界面同样吐中文。`Composer` 根本没引用它。

`/` 选中后写入输入框的仍是 `/{raw}`（`applyTrigger`），这是给模型看的，保持不变。

### 2.2 显示名 + 一句话（三处共用）

约束：中文名 ≤6 字，动词开头优先；中文描述 ≤20 字；英文与中文语义对等，不是字面直译。

`normalizeSkillName` 已把 `-` 收成 `_`，所以 `make-skill` → `make_skill`。另加别名：`dingtalk_channel_connect`（SKILL.md 的 `name:`）→ `dingtalk_channel`。

```ts
// 可抄进 skillPresentation.ts
export type SkillCopy = {
  name: { zh: string; en: string };
  desc: { zh: string; en: string };
};

export const SKILL_COPY: Record<string, SkillCopy> = {
  browser_cdp: {
    name: { zh: "接管浏览器", en: "Take over browser" },
    desc: { zh: "控制已打开的 Chrome", en: "Control your open Chrome" },
  },
  browser_visible: {
    name: { zh: "显示浏览器", en: "Show browser" },
    desc: { zh: "让操作窗口可见", en: "Keep the window visible" },
  },
  channel_message: {
    name: { zh: "发送消息", en: "Send message" },
    desc: { zh: "向指定会话发一条", en: "Send to a chat or group" },
  },
  chat_with_agent: {
    name: { zh: "咨询助手", en: "Ask an agent" },
    desc: { zh: "找另一个助手商量", en: "Consult another assistant" },
  },
  cron: {
    name: { zh: "定时任务", en: "Schedule tasks" },
    desc: { zh: "创建和管理定时任务", en: "Create and manage jobs" },
  },
  dingtalk_channel: {
    name: { zh: "接入钉钉", en: "Connect DingTalk" },
    desc: { zh: "自动配好钉钉通道", en: "Set up the DingTalk channel" },
  },
  dingtalk_channel_connect: {
    name: { zh: "接入钉钉", en: "Connect DingTalk" },
    desc: { zh: "自动配好钉钉通道", en: "Set up the DingTalk channel" },
  },
  docx: {
    name: { zh: "处理 Word", en: "Edit Word" },
    desc: { zh: "创建阅读编辑文档", en: "Create, read, and edit Word" },
  },
  file_reader: {
    name: { zh: "阅读文件", en: "Read files" },
    desc: { zh: "读取并摘要文本", en: "Read and summarize text" },
  },
  guidance: {
    name: { zh: "使用帮助", en: "Get help" },
    desc: { zh: "回答安装配置问题", en: "Answer setup questions" },
  },
  himalaya: {
    name: { zh: "收发邮件", en: "Handle email" },
    desc: { zh: "用邮箱收发和管理", en: "Read and send mail" },
  },
  make_skill: {
    name: { zh: "沉淀技能", en: "Save as skill" },
    desc: { zh: "把做法做成可复用技能", en: "Turn this into a reusable skill" },
  },
  make_plan: {
    name: { zh: "整理计划", en: "Make a plan" },
    desc: { zh: "把需求拆成执行步骤", en: "Break the request into steps" },
  },
  multi_agent_collaboration: {
    name: { zh: "分派协作", en: "Delegate work" },
    desc: { zh: "把任务分给多个助手", en: "Split work across agents" },
  },
  pdf: {
    name: { zh: "处理 PDF", en: "Handle PDF" },
    desc: { zh: "阅读生成处理 PDF", en: "Read, create, and process PDFs" },
  },
  pptx: {
    name: { zh: "编辑幻灯片", en: "Edit slides" },
    desc: { zh: "创建和编辑演示文稿", en: "Create and edit presentations" },
  },
  xlsx: {
    name: { zh: "处理表格", en: "Edit sheets" },
    desc: { zh: "读取生成分析表格", en: "Read, create, and analyze tables" },
  },
  // 文档仍列、仓库已无目录；旧工作区若还在就兜住
  news: {
    name: { zh: "查询新闻", en: "Fetch news" },
    desc: { zh: "查新闻并做摘要", en: "Fetch and summarize news" },
  },
};
```

字数核对（中文名 / 中文描述）：

| raw | 名 | 字 | 描述 | 字 |
|---|---|---|---|---|
| browser_cdp | 接管浏览器 | 5 | 控制已打开的 Chrome | 11 |
| browser_visible | 显示浏览器 | 5 | 让操作窗口可见 | 7 |
| channel_message | 发送消息 | 4 | 向指定会话发一条 | 8 |
| chat_with_agent | 咨询助手 | 4 | 找另一个助手商量 | 8 |
| cron | 定时任务 | 4 | 创建和管理定时任务 | 9 |
| dingtalk_channel | 接入钉钉 | 4 | 自动配好钉钉通道 | 8 |
| docx | 处理 Word | 5 | 创建阅读编辑文档 | 8 |
| file_reader | 阅读文件 | 4 | 读取并摘要文本 | 7 |
| guidance | 使用帮助 | 4 | 回答安装配置问题 | 8 |
| himalaya | 收发邮件 | 4 | 用邮箱收发和管理 | 8 |
| make_skill | 沉淀技能 | 4 | 把做法做成可复用技能 | 10 |
| make_plan | 整理计划 | 4 | 把需求拆成执行步骤 | 9 |
| multi_agent_collaboration | 分派协作 | 4 | 把任务分给多个助手 | 9 |
| pdf | 处理 PDF | 5 | 阅读生成处理 PDF | 8 |
| pptx | 编辑幻灯片 | 5 | 创建和编辑演示文稿 | 9 |
| xlsx | 处理表格 | 4 | 读取生成分析表格 | 8 |
| news | 查询新闻 | 4 | 查新闻并做摘要 | 7 |

成对区分（避免两个浏览器 / 两个「找助手」撞车）：

- `browser_cdp` 接管已开的 Chrome；`browser_visible` 让窗口可见。
- `chat_with_agent` 问一个助手；`multi_agent_collaboration` 把活分给多个。

### 2.3 API（替换现有 `skillDescription(name, fallback)`）

```ts
export function skillDisplayName(
  name: string,
  locale: "zh" | "en",
): string {
  const entry = SKILL_COPY[normalizeSkillName(name)];
  if (entry) return entry.name[locale];
  // 未收录：Title Case，不编中文
  return name.replace(/^mcp__/, "").replaceAll("_", " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

export function skillDescription(
  name: string,
  locale: "zh" | "en",
): string {
  return SKILL_COPY[normalizeSkillName(name)]?.desc[locale] ?? "";
}

export function skillSearchHaystack(name: string, tags: string[] = []): string {
  const key = normalizeSkillName(name);
  const entry = SKILL_COPY[key];
  return [
    name,
    key,
    entry?.name.zh,
    entry?.name.en,
    entry?.desc.zh,
    entry?.desc.en,
    ...tags,
  ]
    .filter(Boolean)
    .join(" ");
}
```

**禁止**再把 SKILL.md 的 `description` 当 fallback。那是模型触发说明（「Use this skill when…」「当用户明确希望连接到已运行的 Chrome…」）。未收录技能：名字走 Title Case，描述走已有 `skills.noDescription`。

若坚持给自定义技能留一条后路：仅当 fallback **不像**触发语（不以 `Use this skill` / `当用户` / `使用本` / `仅在需要` / `用于把` 开头）且 ≤40 字时才用；否则仍走「暂无描述」。默认建议是干脆不用。

### 2.4 raw 名留在哪

| 用途 | 是否可见 | 做法 |
|---|---|---|
| `/` 写入输入框 | 写入 `/{raw} ` | `TriggerItem.value` 继续是 `skill.name`。模型认这个。 |
| `/` 弹层主标题 | 否 | 主标题改 `skillDisplayName`；右侧一行 `skillDescription`（≤20 字）。 |
| `/` 弹层 tooltip | 是 | `title={skill.name}`，悬停才看到 `browser_cdp`。 |
| `/` 与 SkillsView 搜索 | 不可见，参与匹配 | haystack = raw + 中英名 + 中英描述 + tags。打 `cdp` / `接管` / `browser` 都能中。**不要**拿 SKILL.md 长文参与搜索。 |
| 设置页列表 | 否 | 只显示人名。`aria-label` 用人名。 |
| SkillsView 列表 / 池 / Hub | 否 | 主标题人名；副文案一句话。 |
| SkillsView 详情「内部标识」 | 是 | 已有 `skills.internalName`，保留。`title={skill.name}` 也可留。 |
| 后端 / API / 开关请求 | — | 一律 raw，不改。 |

`humanSkillName` 从 `SkillsView.tsx` 挪进 `skillPresentation.ts`（就是上面的 fallback），三处都从这里取，禁止再写一套 Title Case。

`Composer.tsx` 触发映射改为：

```ts
.map((skill) => ({
  value: skill.name, // 写入 /raw
  description: skillDescription(skill.name, locale),
  label: skillDisplayName(skill.name, locale), // TriggerPopover 主标题用这个
  emoji: skill.emoji,
}))
```

`TriggerPopover` 主标题从 `item.value` 改为 `item.label ?? item.value`。`@` 文件项没有 label，继续显示文件名。

设置列表：`{skill.emoji} {skillDisplayName(skill.name, locale)}`。

---

## 3. 以后什么时候配说明文字

1. **先删再问。** 写完把小字拿掉：用户会不会选错、点错、或把灰掉的控件当成坏了？三个都否 → 不配。错了用提交后的错误提示补，不要提前讲课。

2. **含义写进控件，不写在控件底下。** 分段按钮、开关、下拉的选项文案已经说清「是什么」时，行下 description 必删。选项两个字再跟一行解释，是本轮要灭的形态（主题三键、语言两键、审批四档旧 hint）。

3. **只留三类字。** (a) 选错会坏、且选项本身看不出来（协议、沙箱关）；(b) 控件禁用或「看着开了其实没生效」的原因（没密钥、要管理员、平台不支持）；(c) 当前状态值（已保存、连接成功、已启用 12/16）。不是这三类，删。

4. **给人看的和给模型看的分开。** 用户可见名称/描述走 presentation 层；SKILL.md `description`、内部 id、后端枚举不准当主文案。raw id 只用于插入命令、搜索、详情「内部标识」、tooltip。

5. **中英成对，三处一套。** 任何用户可见字符串必须 zh/en 同时落地、语义对等。技能文案改一处，必须同时改 `/` 弹层、设置列表、SkillsView。不要再在页面里私写 Title Case。

---

## 4. 下一手改哪些文件（仍不在本轮改）

- `app/src/lib/i18n.ts`：审批 label；设置缩短 9 条；停用四个 `*Hint`。
- `app/src/views/SettingsView.tsx`：删掉 §1.1 标「删」的 `description=`；技能列表改 `skillDisplayName`。
- `app/src/components/chat/Composer.tsx`：审批去掉 hint 行；`/` 项改走 presentation。
- `app/src/components/chat/TriggerPopover.tsx`：主标题用 `label`。
- `app/src/lib/skillPresentation.ts`：换成 §2.2 / §2.3。
- `app/src/views/SkillsView.tsx`：删本地 `humanSkillName`，搜索改 `skillSearchHaystack`。

本文件是文案源。实现时按表抄，不要再发挥一段「用户可能看不懂」的说明。
