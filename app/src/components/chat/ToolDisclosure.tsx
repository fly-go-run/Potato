import { useState, type MouseEvent, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { Collapse } from "./Collapse";

/**
 * 工具卡统一的受控折叠壳:替代原生 <details>(瞬间开合、无过渡),
 * 开合与执行轨道行共用同一套 qp-collapse 动画。展开语义由行内真实
 * <button>(箭头 + 主内容)承担;「打开文件」等其他交互控件作为兄弟
 * 节点放在 after 里,不嵌套在按钮内。容器整行可点只是指针便利,不带
 * 按钮语义。
 *
 * 行几何是恒定的:运行中与完成态用完全相同的外边距、内边距、最小高度
 * 和箭头尺寸,状态切换只换图标与颜色。以前运行中会渲染成描边卡片,
 * 完成瞬间塌回安静行,整行高度突变 ~10px,在底部吸附滚动下表现为页面
 * 抖动。「卡片感」现在只保留在展开的详情面板里(由 detailClassName 给),
 * 行本身永远是扁平行。
 */
export function ToolDisclosure({
  toggle,
  after,
  toggleGrow = true,
  detailClassName = "",
  children,
}: {
  /** 展开按钮内的主内容(工具名、命令等,不含交互控件)。 */
  toggle: ReactNode;
  /** 行内按钮之后的兄弟内容(路径按钮、时长、Spinner / 状态图标)。 */
  after?: ReactNode;
  /** 展开按钮是否占据剩余宽度;after 里有 flex-1 内容时关掉。 */
  toggleGrow?: boolean;
  /**
   * 详情面板的样式(描边、内边距、滚动上限),由调用方给。这是唯一
   * 允许随状态变化的部分——它只影响展开后的面板,不碰行几何。
   */
  detailClassName?: string;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const toggleOpen = () => setOpen((value) => !value);
  const onButtonClick = (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    toggleOpen();
  };

  return (
    <div className="my-0.5">
      <div
        onClick={toggleOpen}
        className="flex min-h-7 cursor-pointer items-center gap-1.5 rounded-[var(--radius-sm)] px-1.5 py-1 text-xs transition-colors duration-[var(--dur-fast)] hover:bg-fill-hover"
      >
        <button
          type="button"
          aria-expanded={open}
          onClick={onButtonClick}
          className={`flex min-w-0 items-center gap-1.5 text-left ${
            toggleGrow ? "flex-1" : "shrink-0"
          }`}
        >
          <ChevronRight
            size={12}
            className={`shrink-0 text-ink-tertiary transition-transform duration-[var(--dur-fast)] ${
              open ? "rotate-90" : ""
            }`}
          />
          {toggle}
        </button>
        {after}
      </div>
      <Collapse open={open}>
        <div className={detailClassName}>{children}</div>
      </Collapse>
    </div>
  );
}
