import { useState, type MouseEvent, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { Collapse } from "./Collapse";

/**
 * 工具卡统一的受控折叠壳:替代原生 <details>(瞬间开合、无过渡),
 * 开合与执行轨道行共用同一套 qp-collapse 动画。展开语义由行内真实
 * <button>(箭头 + 主内容)承担;「打开文件」等其他交互控件作为兄弟
 * 节点放在 after 里,不嵌套在按钮内。容器整行可点只是指针便利,不带
 * 按钮语义。运行中(card)与完成态(quiet)共用同一实例,状态切换时
 * 展开状态原位保留。
 */
export function ToolDisclosure({
  card,
  toggle,
  after,
  toggleGrow = true,
  detailClassName = "",
  children,
}: {
  /** true = 运行中的描边卡片;false = 完成后的安静行。 */
  card: boolean;
  /** 展开按钮内的主内容(工具名、命令等,不含交互控件)。 */
  toggle: ReactNode;
  /** 行内按钮之后的兄弟内容(路径按钮、时长、状态图标)。 */
  after?: ReactNode;
  /** 展开按钮是否占据剩余宽度;after 里有 flex-1 内容时关掉。 */
  toggleGrow?: boolean;
  /** 详情容器的补充样式(滚动、内边距等),按 card/quiet 由调用方给。 */
  detailClassName?: string;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const toggleOpen = () => setOpen((value) => !value);
  const onButtonClick = (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    toggleOpen();
  };
  const gap = card ? "gap-2" : "gap-1.5";

  return (
    <div
      className={
        card
          ? "my-2 overflow-hidden rounded-[var(--radius-md)] border border-line bg-bubble-tool"
          : "my-0.5"
      }
    >
      <div
        onClick={toggleOpen}
        className={`flex cursor-pointer items-center ${gap} ${
          card
            ? "min-h-9 px-3 py-2 text-xs"
            : "min-h-7 rounded-[var(--radius-sm)] px-1.5 py-1 text-xs transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover"
        }`}
      >
        <button
          type="button"
          aria-expanded={open}
          onClick={onButtonClick}
          className={`flex min-w-0 items-center ${gap} text-left ${
            toggleGrow ? "flex-1" : "shrink-0"
          }`}
        >
          <ChevronRight
            size={card ? 14 : 12}
            className={`shrink-0 text-ink-muted transition-transform duration-[var(--dur-fast)] ${
              open ? "rotate-90" : ""
            }`}
          />
          {toggle}
        </button>
        {after}
      </div>
      <Collapse open={open}>
        <div
          className={
            card ? `border-t border-line ${detailClassName}` : detailClassName
          }
        >
          {children}
        </div>
      </Collapse>
    </div>
  );
}
