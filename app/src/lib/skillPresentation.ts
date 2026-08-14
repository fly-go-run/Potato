import type { Language } from "./i18n";

/**
 * 技能的用户可读文案。后端的 name 是工程标识符(browser_cdp),
 * description 是写给模型看的英文触发 prompt("Use this skill when…"),
 * 都不能直接对用户展示;这里维护面向人的中英文显示名与一句话描述。
 * 未收录的技能回落到原始名/原描述——宁可暴露一个待补条目,不伪装。
 *
 * 展示名 ≤6 个汉字,像功能不像变量;原始名保留在 title(悬停可见),
 * 且搜索匹配同时命中原始名与展示名。
 */
interface SkillCopy {
  zh: { name: string; description: string };
  en: { name: string; description: string };
}

const SKILL_COPY: Record<string, SkillCopy> = {
  qa_source_index: {
    zh: { name: "资料索引", description: "检索团队问答资料库并给出出处。" },
    en: { name: "QA index", description: "Search the QA source library with citations." },
  },
  browser_cdp: {
    zh: { name: "接管浏览器", description: "连接你已打开的 Chrome，自动完成网页操作。" },
    en: { name: "Take over Chrome", description: "Attach to your running Chrome and automate it." },
  },
  browser_visible: {
    zh: { name: "显示浏览器", description: "让自动化浏览器以可见窗口运行。" },
    en: { name: "Visible browser", description: "Run browser automation in a visible window." },
  },
  channel_message: {
    zh: { name: "发送消息", description: "主动向用户、会话或群发一条消息。" },
    en: { name: "Send a message", description: "Proactively message a user, session, or channel." },
  },
  chat_with_agent: {
    zh: { name: "咨询同伴", description: "拉另一个 agent 一起讨论或帮忙。" },
    en: { name: "Ask another agent", description: "Consult another agent for help." },
  },
  cron: {
    zh: { name: "定时任务", description: "创建和管理定时运行的任务。" },
    en: { name: "Scheduled tasks", description: "Create and manage scheduled jobs." },
  },
  dingtalk_channel: {
    zh: { name: "接入钉钉", description: "自动完成钉钉消息通道的配置。" },
    en: { name: "DingTalk setup", description: "Set up the DingTalk message channel." },
  },
  docx: {
    zh: { name: "Word 文档", description: "创建、阅读和编辑 Word 文档。" },
    en: { name: "Word documents", description: "Create, read, and edit Word documents." },
  },
  file_reader: {
    zh: { name: "读取文件", description: "读取并摘要文本类文件。" },
    en: { name: "Read files", description: "Read and summarize text files." },
  },
  guidance: {
    zh: { name: "使用帮助", description: "回答 Potato 的安装与配置问题。" },
    en: { name: "Help", description: "Answer Potato setup and usage questions." },
  },
  himalaya: {
    zh: { name: "收发邮件", description: "通过邮箱账号收发和管理邮件。" },
    en: { name: "Email", description: "Send and manage email via your account." },
  },
  make_skill: {
    zh: { name: "沉淀技能", description: "把本次会话的做法保存为可复用技能。" },
    en: { name: "Save as skill", description: "Turn this session's approach into a reusable skill." },
  },
  make_plan: {
    zh: { name: "制定计划", description: "把需求整理成分步可执行的计划。" },
    en: { name: "Make a plan", description: "Turn a request into an actionable step plan." },
  },
  multi_agent_collaboration: {
    zh: { name: "多人协作", description: "把任务分发给多个 agent 协作完成。" },
    en: { name: "Multi-agent", description: "Fan a task out to multiple agents." },
  },
  pdf: {
    zh: { name: "PDF 文档", description: "阅读、生成和处理 PDF 文件。" },
    en: { name: "PDF", description: "Read, create, and process PDF files." },
  },
  pptx: {
    zh: { name: "幻灯片", description: "创建和编辑 PowerPoint 演示文稿。" },
    en: { name: "Slides", description: "Create and edit PowerPoint decks." },
  },
  xlsx: {
    zh: { name: "表格", description: "读取、生成与分析 Excel 表格。" },
    en: { name: "Spreadsheets", description: "Read, create, and analyze Excel files." },
  },
};

function normalizeSkillName(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, "_");
}

/** 面向人的显示名;未收录回落原始名。 */
export function skillDisplayName(name: string, language: Language): string {
  return SKILL_COPY[normalizeSkillName(name)]?.[language].name ?? name;
}

/** 面向人的一句话描述;未收录回落传入的原描述。 */
export function skillDescription(
  name: string,
  fallback: string,
  language: Language = "zh",
): string {
  return SKILL_COPY[normalizeSkillName(name)]?.[language].description ?? fallback;
}
