/**
 * 模型名产品化。gpt-5.6-terra 是给 API 看的 id,不是给人看的名字。
 * 优先序:后端 ModelInfo.name(且 ≠ id)→ 白名单 → 通用美化兜底。
 * 只有白名单允许删厂牌/重排词序(terra 才是身份,gpt 是通用词);
 * 兜底永不删首段(gpt-5.6 → GPT 5.6,不是 5.6)。raw id 保留在
 * tooltip 与设置的模型管理(管理场景要可精确复制)。
 */

const WHITELIST: Record<string, string> = {
  "gpt-5.6-terra": "5.6 Terra",
  "gpt-5.6-sol": "5.6 Sol",
  "gpt-5.6-luna": "5.6 Luna",
  "grok-4.6": "Grok 4.6",
  "grok-4.5": "Grok 4.5",
  "deepseek-v4-flash": "DeepSeek V4 Flash",
  "deepseek-v4-pro": "DeepSeek V4 Pro",
  "deepseek-v3.2": "DeepSeek V3.2",
};

const ACRONYMS = new Set(["gpt", "glm", "llm"]);
const TRAILING_CHANNEL = new Set(["preview", "latest", "beta", "alpha", "exp"]);

function titleToken(token: string): string {
  // glm4 这类「缩写+数字」也按缩写大写
  const stem = token.toLowerCase().replace(/\d+$/, "");
  if (ACRONYMS.has(token.toLowerCase()) || ACRONYMS.has(stem)) {
    return token.toUpperCase();
  }
  // 字母+数字粘连成一词:qwen3 → Qwen3,v4 → V4
  if (/^v\d/i.test(token)) return token.toUpperCase();
  return token.charAt(0).toUpperCase() + token.slice(1);
}

function prettify(id: string): string {
  // org/model 路径取末段
  let s = id.split("/").pop() ?? id;
  // 日期后缀扔掉(tooltip 里还有全称)
  s = s.replace(/-\d{8}$/, "").replace(/-\d{4}-\d{2}-\d{2}$/, "");
  // 中文名:只把分隔符换空格,拉丁词首字母大写,禁止整串 Title Case
  if (/[一-鿿]/.test(s)) {
    return s
      .split(/[-_]+/)
      .map((part) => (/^[a-z0-9.]+$/i.test(part) ? titleToken(part) : part))
      .join(" ");
  }
  const tokens = s.split(/[-_:]+/).filter(Boolean);
  if (tokens.length === 0) return id;
  return tokens.map(titleToken).join(" ");
}

/** 面向人的模型名。backendName 来自 ModelInfo.name,优先且不再加工。 */
export function prettyModelName(id: string, backendName?: string | null): string {
  if (backendName && backendName !== id) return backendName;
  const key = id.toLowerCase();
  if (WHITELIST[key]) return WHITELIST[key];
  const pretty = prettify(id);
  // 尾部渠道词(preview/latest)保留——那是档位不是装饰,已由 prettify 保序
  void TRAILING_CHANNEL;
  return pretty;
}
