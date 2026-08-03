import type { ReactNode } from "react";

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
