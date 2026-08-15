import type { ReactNode } from "react";
import { Collapse } from "./Collapse";
import { TrackRow } from "./TrackRow";

/**
 * 多条同族工具的 fold-row:摘要在 Collapse 外,原始卡进 struct 档。
 * 思考行不用这个组件。
 */
export function StepGroupRow({
  icon,
  summary,
  open,
  keepMounted,
  onToggle,
  shimmer,
  children,
}: {
  icon?: ReactNode;
  summary: ReactNode;
  open: boolean;
  keepMounted: boolean;
  onToggle: () => void;
  shimmer?: boolean;
  children: ReactNode;
}) {
  return (
    <div>
      <TrackRow
        open={open}
        onToggle={onToggle}
        icon={icon}
        shimmer={shimmer}
      >
        {summary}
      </TrackRow>
      <Collapse open={open} keepMounted={keepMounted} struct>
        {children}
      </Collapse>
    </div>
  );
}
