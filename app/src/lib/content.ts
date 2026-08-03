import type { ContentBlock } from "./protocol/types";

export function textFromContent(content: ContentBlock[]): string {
  return content
    .filter(
      (part): part is Extract<ContentBlock, { type: "text" }> =>
        part.type === "text" && typeof part.text === "string",
    )
    .map((part) => part.text)
    .join("");
}
