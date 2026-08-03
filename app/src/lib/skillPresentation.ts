/**
 * 技能的用户可读文案。后端的 description 是写给模型看的英文触发
 * prompt("Use this skill when…"),不能直接对用户展示;这里维护
 * 面向人的中文一句话。未收录的技能回落到原描述。
 */
const SKILL_DESCRIPTIONS: Record<string, string> = {
  qa_source_index: "把用户问题映射到官方文档条目，用于精准答疑。",
  browser_cdp: "连接你已打开的 Chrome 浏览器，执行网页自动化操作。",
  browser_visible: "让浏览器以可见窗口方式运行，便于观察操作过程。",
  channel_message: "主动向指定用户、会话或群发送一条消息。",
  chat_with_agent: "咨询或协同另一个 agent 一起完成任务。",
  cron: "创建和管理定时任务。",
  dingtalk_channel: "自动完成钉钉消息通道的接入配置。",
  docx: "创建、阅读和编辑 Word 文档。",
  file_reader: "读取并摘要文本类文件。",
  guidance: "回答 Potato 的安装与配置问题。",
  himalaya: "通过 IMAP/SMTP 收发和管理邮件。",
  make_skill: "把当前会话的做法沉淀成可复用的技能。",
  make_plan: "把需求整理成清晰可执行的分步计划。",
  multi_agent_collaboration: "需要多个 agent 协作时进行任务分发。",
  pdf: "阅读、生成和处理 PDF 文件。",
  pptx: "创建和编辑 PowerPoint 演示文稿。",
  xlsx: "读取、生成与分析 Excel 表格。",
};

function normalizeSkillName(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, "_");
}

export function skillDescription(name: string, fallback: string): string {
  return SKILL_DESCRIPTIONS[normalizeSkillName(name)] ?? fallback;
}
