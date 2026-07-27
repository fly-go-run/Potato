import { useEffect, useState, type ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { ThemedToken } from "@shikijs/types";
import { highlightCode, isSupportedLanguage } from "../../lib/highlight";

interface MarkdownProps {
  children: string;
}

export function Markdown({ children }: MarkdownProps) {
  return (
    <div className="min-w-0 text-sm leading-7 text-ink">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
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
          a: ({ children: value, href }) => (
            <a
              href={href}
              target="_blank"
              rel="noreferrer"
              className="text-accent underline decoration-accent/40 underline-offset-2 hover:decoration-accent"
            >
              {value}
            </a>
          ),
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
    if (!isSupportedLanguage(language)) {
      setLines(null);
      return () => {
        active = false;
      };
    }
    void highlightCode(code, language)
      .then((result) => {
        if (active) setLines(result);
      })
      .catch(() => {
        if (active) setLines(null);
      });
    return () => {
      active = false;
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

function tokenClass(token: ThemedToken) {
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

export function textFromContent(
  content: Array<{ type: string; text?: string }>,
): string {
  return content
    .filter(
      (part): part is { type: "text"; text: string } =>
        part.type === "text" && typeof part.text === "string",
    )
    .map((part) => part.text)
    .join("");
}

export function JsonView({ value }: { value: unknown }) {
  let rendered: ReactNode;
  try {
    rendered =
      typeof value === "string"
        ? JSON.stringify(JSON.parse(value), null, 2)
        : JSON.stringify(value, null, 2);
  } catch {
    rendered = String(value ?? "");
  }
  return (
    <pre className="overflow-x-auto whitespace-pre-wrap break-words font-mono text-xs leading-6 text-ink-secondary">
      {rendered}
    </pre>
  );
}
