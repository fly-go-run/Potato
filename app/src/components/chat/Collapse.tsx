import { useEffect, useState, type ReactNode } from "react";

/** 与 .qp-collapse / .qp-collapse-struct 的行高过渡时长保持一致。 */
const COLLAPSE_MS = 200;
const STRUCT_COLLAPSE_MS = 280;

/**
 * 统一折叠容器:展开时挂载内容并以行高过渡展开;收起时播完收口动画
 * 再卸载内容——保住原位收口动效的同时,折叠的历史详情不常驻 DOM(长
 * 会话渲染成本),也不会被 Tab/读屏聚焦到不可见控件。
 */
export function Collapse({
  open,
  keepMounted = false,
  struct = false,
  className,
  children,
}: {
  open: boolean;
  /**
   * 收起后仍保持子树挂载(仅 inert + 0fr 隐藏)。用于需要跨折叠保留
   * 子组件状态的场景(如轨道条目自身的展开状态);内容重的详情不要开。
   */
  keepMounted?: boolean;
  /**
   * 结构折叠档:整段时间线的轮次收口用。高度过渡放慢到 --dur-struct
   * 并叠加内容淡出,读作「整理归档」;条目级的开合不要用这一档。
   */
  struct?: boolean;
  className?: string;
  children: ReactNode;
}) {
  const [exiting, setExiting] = useState(false);
  // 关闭发生的当次 render 就要同步置位 exiting(render 阶段调整状态,
  // prevOpen 用 state 而非 ref:并发渲染丢弃重试时 ref 不会回滚),
  // 否则子树会先卸载、effect 跑完才重挂进已是 0fr 的容器:收口动画
  // 跳变,子组件(如展开中的思考正文)状态也会丢失。
  const [prevOpen, setPrevOpen] = useState(open);
  if (open !== prevOpen) {
    setPrevOpen(open);
    setExiting(!open);
  }
  const durationMs = struct ? STRUCT_COLLAPSE_MS : COLLAPSE_MS;
  useEffect(() => {
    if (open || !exiting) return;
    // reduced-motion 下没有过渡,不为看不见的动画保留 DOM。
    if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
      setExiting(false);
      return;
    }
    const timer = window.setTimeout(() => setExiting(false), durationMs + 60);
    return () => window.clearTimeout(timer);
  }, [open, exiting, durationMs]);
  const mounted = keepMounted || open || exiting;
  // 收口动画期间内容仍在 DOM,inert 把它从焦点顺序与无障碍树中移除。
  // React 18 的 JSX 类型没有 inert prop,以字符串属性形式当次提交。
  const inertProps = open ? undefined : ({ inert: "" } as object);
  const closedProps = !open && struct ? { "data-closed": "" } : undefined;
  return (
    <div
      className={`qp-collapse${struct ? " qp-collapse-struct" : ""}`}
      style={{ gridTemplateRows: open ? "1fr" : "0fr" }}
      {...closedProps}
    >
      <div className={className} {...inertProps}>
        {mounted ? children : null}
      </div>
    </div>
  );
}
