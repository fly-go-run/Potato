/** codex 式 ± 统计:等宽、绿加红减,0 的一侧不显示。 */
export function ChangeStat({
  additions,
  deletions,
}: {
  additions: number;
  deletions: number;
}) {
  if (additions === 0 && deletions === 0) return null;
  return (
    <span className="flex shrink-0 items-center gap-1.5 font-mono text-[12px] tabular-nums">
      {additions > 0 && <span className="text-ok">+{additions}</span>}
      {deletions > 0 && <span className="text-danger">-{deletions}</span>}
    </span>
  );
}
