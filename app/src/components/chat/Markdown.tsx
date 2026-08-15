import { FileText } from "lucide-react";
import { useEffect, useState } from "react";
import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import type { ThemedToken } from "@shikijs/types";
import { filePreviewUrl } from "../../lib/api";
import { openLocalPathWithSystem } from "../../lib/desktop";
import { highlightCode, isSupportedLanguage } from "../../lib/highlight";

interface MarkdownProps {
  children: string;
  /**
   * 相对资源解析:文件预览等场景把 `./img.png` 之类的相对引用
   * 转成可加载的地址。绝对/协议地址原样返回。
   */
  transformUrl?: (url: string) => string;
  /** Resolve assistant-authored Markdown links to previewable local files. */
  resolveFilePath?: (url: string) => string | null;
  onOpenFile?: (path: string) => void;
}

export function Markdown({
  children,
  transformUrl,
  resolveFilePath,
  onOpenFile,
}: MarkdownProps) {
  return (
    <div className="min-w-0 text-[15px] leading-[1.75] text-ink">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        urlTransform={
          transformUrl || resolveFilePath
            ? (url) => {
                // Keep a recognized file reference intact until the custom
                // renderer can replace it with the authenticated preview URL.
                if (resolveFilePath?.(url)) return url;
                const safeUrl = defaultUrlTransform(url);
                return transformUrl ? transformUrl(safeUrl) : safeUrl;
              }
            : defaultUrlTransform
        }
        components={{
          p: ({ children: value }) => (
            <p className="my-2 first:mt-0 last:mb-0">{value}</p>
          ),
          h1: ({ children: value }) => (
            <h1 className="mb-3 mt-6 text-xl font-semibold first:mt-0">
              {value}
            </h1>
          ),
          h2: ({ children: value }) => (
            <h2 className="mb-2 mt-5 text-lg font-semibold first:mt-0">
              {value}
            </h2>
          ),
          h3: ({ children: value }) => (
            <h3 className="mb-2 mt-4 font-semibold first:mt-0">{value}</h3>
          ),
          ul: ({ children: value }) => (
            <ul className="my-2 list-disc space-y-1 pl-5">{value}</ul>
          ),
          ol: ({ children: value }) => (
            <ol className="my-2 list-decimal space-y-1 pl-5">{value}</ol>
          ),
          blockquote: ({ children: value }) => (
            <blockquote className="my-3 border-l-2 border-line-strong pl-4 text-ink-secondary">
              {value}
            </blockquote>
          ),
          a: ({ children: value, href }) => {
            const filePath = href ? resolveFilePath?.(href) : null;
            if (filePath && onOpenFile) {
              return (
                <a
                  href={filePreviewUrl(filePath)}
                  target="_blank"
                  rel="noreferrer"
                  title={filePath}
                  onClick={(event) => {
                    if (
                      event.button !== 0 ||
                      event.metaKey ||
                      event.ctrlKey ||
                      event.shiftKey ||
                      event.altKey
                    ) {
                      return;
                    }
                    event.preventDefault();
                    // 桌面壳优先交给系统默认应用(如 PDF 阅读器);
                    // 浏览器模式或打开失败回落应用内预览面板。
                    void openLocalPathWithSystem(filePath).then((opened) => {
                      if (!opened) onOpenFile(filePath);
                    });
                  }}
                  className="inline-flex items-center gap-1 rounded-[4px] text-accent underline decoration-accent/40 underline-offset-2 transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover hover:decoration-accent"
                >
                  <FileText size={14} strokeWidth={1.8} className="shrink-0" aria-hidden />
                  {value}
                </a>
              );
            }
            return (
              <a
                href={href}
                target="_blank"
                rel="noreferrer"
                className="text-accent underline decoration-accent/40 underline-offset-2 hover:decoration-accent"
              >
                {value}
              </a>
            );
          },
          table: ({ children: value }) => (
            <div className="my-3 overflow-x-auto">
              <table className="w-full border-collapse text-left text-sm">
                {value}
              </table>
            </div>
          ),
          th: ({ children: value }) => (
            <th className="border-b border-line px-3 py-2 font-medium">
              {value}
            </th>
          ),
          td: ({ children: value }) => (
            <td className="border-b border-line px-3 py-2 align-top">
              {value}
            </td>
          ),
          pre: ({ children: value }) => <>{value}</>,
          code: ({ children: value, className }) => {
            const code = String(value).replace(/\n$/, "");
            const match = /language-([\w-]+)/.exec(className ?? "");
            const isBlock = Boolean(match) || code.includes("\n");
            if (!isBlock) {
              return (
                <code className="rounded-sm bg-bubble-tool px-1.5 py-0.5 font-mono text-[0.9em] text-ink">
                  {code}
                </code>
              );
            }
            return <HighlightedCode code={code} language={match?.[1]} />;
          },
        }}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}

interface HighlightedCodeProps {
  code: string;
  language?: string;
}

function HighlightedCode({ code, language }: HighlightedCodeProps) {
  const [lines, setLines] = useState<ThemedToken[][] | null>(null);

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout> | null = null;
    setLines(null);
    if (!isSupportedLanguage(language)) {
      return () => {
        active = false;
      };
    }
    timer = setTimeout(() => {
      timer = null;
      void highlightCode(code, language)
        .then((result) => {
          if (active) setLines(result);
        })
        .catch(() => {
          if (active) setLines(null);
        });
    }, 200);
    return () => {
      active = false;
      if (timer !== null) clearTimeout(timer);
    };
  }, [code, language]);

  return (
    <pre className="my-3 overflow-x-auto rounded-md bg-bubble-tool px-4 py-3 font-mono text-xs leading-6 text-ink">
      <code>
        {lines
          ? lines.map((line, lineIndex) => (
              <span key={lineIndex} className="block min-h-6">
                {line.map((token, tokenIndex) => (
                  <span
                    key={`${lineIndex}-${tokenIndex}`}
                    className={tokenClass(token)}
                  >
                    {token.content}
                  </span>
                ))}
              </span>
            ))
          : code}
      </code>
    </pre>
  );
}

export function tokenClass(token: ThemedToken) {
  const scopes = (token.explanation ?? [])
    .flatMap((entry) => entry.scopes)
    .map((scope) => scope.scopeName)
    .join(" ");
  if (/(comment|punctuation\.definition\.comment)/.test(scopes)) {
    return "text-ink-muted";
  }
  if (/(invalid|illegal)/.test(scopes)) return "text-danger";
  if (/(string|regexp)/.test(scopes)) return "text-ok";
  if (/(constant\.numeric|number|boolean)/.test(scopes)) return "text-warn";
  if (/(keyword|storage|entity\.name\.tag|support\.function)/.test(scopes)) {
    return "text-accent";
  }
  if (/(entity\.name|variable\.language|support\.type)/.test(scopes)) {
    return "text-ink-secondary";
  }
  return "text-ink";
}
