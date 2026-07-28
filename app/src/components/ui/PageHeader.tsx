import type { ReactNode } from "react";

/**
 * 统一页头：固定标题字阶（display 级）、副标题、右侧操作区，
 * 让所有 view 的标题左边缘与节奏一致（消除 6 页各写各的）。
 * 容器宽度由页面外层 PageContainer 决定。
 */
export interface PageHeaderProps {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
}

export function PageHeader({ title, subtitle, actions }: PageHeaderProps) {
  return (
    <header className="mb-8 flex items-start justify-between gap-4">
      <div className="min-w-0">
        <h1 className="text-[26px] font-semibold leading-9 tracking-tight text-ink">
          {title}
        </h1>
        {subtitle && (
          <p className="mt-1.5 text-sm text-ink-tertiary">{subtitle}</p>
        )}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </header>
  );
}

export type PageWidth = "reading" | "wide";

/**
 * 统一页面容器宽度：阅读型（Chat/Settings/Memory/Inbox）用 reading，
 * 宽表型（Crons/Skills）用 wide。杜绝此前 3xl/4xl/5xl/6xl 四种混用。
 */
export function PageContainer({
  width = "reading",
  children,
}: {
  width?: PageWidth;
  children: ReactNode;
}) {
  return (
    <div className="h-full overflow-y-auto">
      <div
        className={
          "mx-auto px-6 py-8 sm:px-10 " +
          (width === "wide" ? "max-w-5xl" : "max-w-3xl")
        }
      >
        {children}
      </div>
    </div>
  );
}
