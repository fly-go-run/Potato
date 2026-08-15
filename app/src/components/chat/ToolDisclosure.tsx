import { useState, type MouseEvent, type ReactNode } from "react";
import { Collapse } from "./Collapse";
import { TrackRowChevron } from "./TrackRow";

/**
 * 工具卡统一的受控折叠壳:替代原生 <details>(瞬间开合、无过渡),
 * 开合与执行轨道行共用同一套 qp-collapse 动画。展开语义由行内真实
 * <button>(主内容)承担;「打开文件」等其他交互控件作为兄弟
 * 节点放在 after 里,不嵌套在按钮内。容器整行可点只是指针便利,不带
 * 按钮语义。
 *
 * 行是安静文本行:无静息 chevron、无 hover 灰底。卡片感只留在
 * 展开后的详情(由 detailClassName 给),行本身永远是扁平行。
 */
export function ToolDisclosure({
  toggle,
  after,
  trailing,
  toggleGrow = true,
  detailClassName = "",
  failed = false,
  children,
}: {
  /** 展开按钮内的主内容(工具名、命令等,不含交互控件)。 */
  toggle: ReactNode;
  /** 行内按钮之后的兄弟内容(路径按钮、时长、Spinner / 状态图标)。 */
  after?: ReactNode;
  /** 依赖开合态的行尾控件(如「在侧栏打开」);放在 chevron 前。 */
  trailing?: (open: boolean) => ReactNode;
  /** 展开按钮是否占据剩余宽度;after 里有 flex-1 内容时关掉。 */
  toggleGrow?: boolean;
  /**
   * 详情面板的样式(描边、内边距、滚动上限),由调用方给。这是唯一
   * 允许随状态变化的部分——它只影响展开后的面板,不碰行几何。
   */
  detailClassName?: string;
  failed?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const toggleOpen = () => setOpen((value) => !value);
  const onButtonClick = (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    toggleOpen();
  };

  return (
    <div>
      <div
        onClick={toggleOpen}
        className={`group flex cursor-pointer items-center gap-1.5 py-1 text-[13px] transition-colors duration-[var(--dur-fast)] ${
          failed ? "text-danger" : "text-ink-secondary hover:text-ink"
        }`}
      >
        <button
          type="button"
          aria-expanded={open}
          onClick={onButtonClick}
          className={`flex min-w-0 items-center gap-1.5 text-left ${
            toggleGrow ? "flex-1" : "shrink-0"
          }`}
        >
          {toggle}
        </button>
        {after}
        {trailing?.(open)}
        <TrackRowChevron open={open} failed={failed} />
      </div>
      <Collapse open={open}>
        <div className={detailClassName}>{children}</div>
      </Collapse>
    </div>
  );
}
