/**
 * 运行状态指示环。全应用统一的「进行中」表达:只有当前活动步骤
 * 使用它,历史步骤一律换成静态图标,避免整屏闪烁。
 */
export function Spinner({
  size = 14,
  className = "",
}: {
  size?: number;
  className?: string;
}) {
  return (
    <span
      aria-hidden
      className={`qp-spinner ${className}`}
      style={{ width: size, height: size }}
    />
  );
}
