import type { Language } from "./i18n";

/**
 * 技能的用户可读文案(唯一来源:/ 弹层、设置列表、SkillsView 三处共用)。
 * 后端 name 是工程标识符(browser_cdp),SKILL.md 的 description 是写给
 * 模型看的触发 prompt——都不许当用户主文案。未收录技能:名字回落
 * Title Case,描述回落空(由调用方显示「暂无描述」),不拿触发语凑数。
 * raw 名保留在:写回输入框的 token、搜索匹配、tooltip、详情内部标识。
 */
interface SkillCopy {
  name: { zh: string; en: string };
  desc: { zh: string; en: string };
}

const SKILL_COPY: Record<string, SkillCopy> = {
  qa_source_index: {
    name: { zh: "资料索引", en: "QA index" },
    desc: { zh: "检索问答资料库", en: "Search the QA library" },
  },
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
    name: { zh: "Word 文档", en: "Word documents" },
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
    name: { zh: "PDF 文档", en: "PDF documents" },
    desc: { zh: "阅读生成处理 PDF", en: "Read, create, and process PDFs" },
  },
  pptx: {
    name: { zh: "幻灯片", en: "Slides" },
    desc: { zh: "创建和编辑演示文稿", en: "Create and edit presentations" },
  },
  xlsx: {
    name: { zh: "表格", en: "Spreadsheets" },
    desc: { zh: "读取生成分析表格", en: "Read, create, and analyze tables" },
  },
  news: {
    name: { zh: "查询新闻", en: "Fetch news" },
    desc: { zh: "查新闻并做摘要", en: "Fetch and summarize news" },
  },
};

function normalizeSkillName(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, "_");
}

/** 面向人的显示名;未收录回落 Title Case(不编造中文)。 */
export function skillDisplayName(name: string, language: Language): string {
  const entry = SKILL_COPY[normalizeSkillName(name)];
  if (entry) return entry.name[language];
  return name
    .replace(/^mcp__/, "")
    .replaceAll("_", " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

/** 面向人的一句话描述;未收录返回空串(调用方显示「暂无描述」)。 */
export function skillDescription(name: string, language: Language): string {
  return SKILL_COPY[normalizeSkillName(name)]?.desc[language] ?? "";
}

/** 搜索干草堆:raw + 中英名 + 中英描述 + tags;不含 SKILL.md 长文。 */
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
