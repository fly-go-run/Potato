export const BOTTOM_THRESHOLD_PX = 80;

export interface ScrollPosition {
  scrollHeight: number;
  scrollTop: number;
  clientHeight: number;
}

export function isAtBottom(
  position: ScrollPosition,
  threshold = BOTTOM_THRESHOLD_PX,
): boolean {
  return (
    position.scrollHeight - position.scrollTop - position.clientHeight <=
    threshold
  );
}
