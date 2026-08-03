import { FileText } from "lucide-react";
import { lazy, Suspense } from "react";
import { filePreviewUrl } from "../../lib/api";
import { useTranslation } from "../../lib/i18n";
import type { ContentBlock } from "../../lib/protocol/types";

const Markdown = lazy(() =>
  import("./Markdown").then((module) => ({ default: module.Markdown })),
);

export function MessageContent({
  content,
  markdown,
}: {
  content: ContentBlock[];
  markdown: boolean;
}) {
  const { t } = useTranslation();

  return (
    <div className="space-y-2">
      {content.map((part, index) => {
        if (part.type === "text" && part.text) {
          return markdown ? (
            <Suspense
              key={index}
              fallback={
                <div className="whitespace-pre-wrap break-words text-sm leading-6 text-ink">
                  {part.text}
                </div>
              }
            >
              <Markdown>{part.text}</Markdown>
            </Suspense>
          ) : (
            <div
              key={index}
              className="whitespace-pre-wrap break-words text-sm leading-6 text-ink"
            >
              {part.text}
            </div>
          );
        }

        if (part.type === "image" && part.image_url) {
          return (
            <a
              key={index}
              href={filePreviewUrl(part.image_url)}
              target="_blank"
              rel="noreferrer"
              className="block overflow-hidden rounded-md border border-line bg-bubble-tool"
              title={t("attachment.preview", {
                name: filenameFromPath(part.image_url),
              })}
            >
              <img
                src={filePreviewUrl(part.image_url)}
                alt={
                  filenameFromPath(part.image_url) || t("attachment.imageAlt")
                }
                className="max-h-80 w-full object-contain"
              />
            </a>
          );
        }

        if (part.type === "file" && part.file_url) {
          const filename =
            part.filename ||
            filenameFromPath(part.file_url) ||
            t("attachment.file");
          return (
            <a
              key={index}
              href={filePreviewUrl(part.file_url)}
              target="_blank"
              rel="noreferrer"
              className="inline-flex max-w-full items-center gap-2 rounded-md border border-line bg-bubble-tool px-3 py-2 text-xs text-ink-secondary transition-colors hover:border-line-strong hover:text-ink"
              title={t("attachment.preview", { name: filename })}
            >
              <FileText size={15} className="shrink-0" />
              <span className="truncate">{filename}</span>
            </a>
          );
        }
        return null;
      })}
    </div>
  );
}

function filenameFromPath(path: string): string {
  const value = path.split(/[?#]/, 1)[0]?.split("/").at(-1) ?? "";
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}
