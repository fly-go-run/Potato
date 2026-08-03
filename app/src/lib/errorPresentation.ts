import { ApiError } from "./api";
import type { TranslationKey } from "./i18n";

export interface ErrorPresentation {
  /** 面向用户的一句话概括(i18n key)。 */
  summaryKey: TranslationKey;
  /** 原始错误文本,放技术详情/悬停,不做主文案。 */
  detail: string;
}

/**
 * 统一错误呈现:按状态码映射成"发生了什么"的人话,
 * 原始信息(英文/内部路径/堆栈措辞)降级为技术详情。
 * 各页面不要再把 Error.message 直接塞给用户。
 */
export function presentError(error: unknown): ErrorPresentation {
  const detail = error instanceof Error ? error.message : String(error ?? "");
  if (error instanceof ApiError) {
    if (error.status === 401 || error.status === 403) {
      return { summaryKey: "error.auth", detail };
    }
    if (error.status === 404) {
      return { summaryKey: "error.notFound", detail };
    }
    if (error.status === 429) {
      return { summaryKey: "error.rateLimited", detail };
    }
    if (error.status >= 500) {
      return { summaryKey: "error.server", detail };
    }
    return { summaryKey: "error.generic", detail };
  }
  // fetch 网络层失败(后端没起/断连)是 TypeError
  if (error instanceof TypeError) {
    return { summaryKey: "error.network", detail };
  }
  return { summaryKey: "error.generic", detail };
}
