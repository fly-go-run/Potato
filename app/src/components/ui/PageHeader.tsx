import type { ReactNode } from "react";

/**
 * 统一页头：桌面 app 的页面身份主要来自导航选中态，内容区不再重复喊一遍，
 * 因此标题只用 19px 工具级字阶（不是 display 级），下边距 24px。
 * 容器宽度由页面外层 PageContainer 决定。
 */
export interface PageHeaderProps {
  title: string;
  subtitle?: string;
  /**
   * 副标题是否展示。副标题对第二次访问的用户是零信息，
   * 建议调用方只在页面空态/首次访问时传 true（如 `showSubtitle={items.length === 0}`）。
   * 默认 true 以兼容尚未接线的页面。
   */
  showSubtitle?: boolean;
  actions?: ReactNode;
}

export function PageHeader({
  title,
  subtitle,
  showSubtitle = true,
  actions,
}: PageHeaderProps) {
  return (
    <header className="mb-6 flex items-start justify-between gap-4">
      <div className="min-w-0">
        <h1 className="text-[19px] font-semibold leading-7 tracking-tight text-ink">
          {title}
        </h1>
        {subtitle && showSubtitle && (
          <p className="mt-1 text-[13px] leading-5 text-ink-tertiary">
            {subtitle}
          </p>
        )}
      </div>
      {actions && (
        <div className="flex shrink-0 items-center gap-2">{actions}</div>
      )}
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
          "mx-auto px-6 py-6 sm:px-10 " +
          (width === "wide" ? "max-w-5xl" : "max-w-3xl")
        }
      >
        {children}
      </div>
    </div>
  );
}
