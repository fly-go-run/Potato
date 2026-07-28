/** 轻量 className 合并（无依赖）：过滤假值并以空格连接。 */
export function cn(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}
