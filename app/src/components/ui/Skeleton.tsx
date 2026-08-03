import { cn } from "../../lib/cn";

/** 骨架块：取代各页裸文字「加载中…」。 */
export function Skeleton({ className }: { className?: string }) {
  return (
    <div
      className={cn(
        "animate-pulse rounded-[var(--radius-sm)] bg-fill-active",
        className,
      )}
    />
  );
}

/** 列表行骨架：用于 Sidebar / 记忆 / 技能等列表加载态。 */
export function SkeletonRows({ rows = 5 }: { rows?: number }) {
  return (
    <div className="space-y-2">
      {Array.from({ length: rows }).map((_, index) => (
        <div key={index} className="flex items-center gap-3 px-1 py-2">
          <Skeleton className="h-8 w-8 rounded-[var(--radius-md)]" />
          <div className="flex-1 space-y-1.5">
            <Skeleton className="h-3 w-2/5" />
            <Skeleton className="h-2.5 w-3/5" />
          </div>
        </div>
      ))}
    </div>
  );
}
