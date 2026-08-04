export const IME_COMMIT_GRACE_MS = 50;

type KeyboardEventLike = Pick<
  KeyboardEvent,
  "key" | "isComposing" | "keyCode" | "timeStamp"
>;

/**
 * WebKit may fire the Enter keydown that commits an IME composition after
 * compositionend, when isComposing is already false. Treat that same-keystroke
 * keydown as part of the composition so it cannot submit the composer.
 */
export function isImeCommitEnter(
  event: KeyboardEventLike,
  composing: boolean,
  compositionEndAt: number,
): boolean {
  if (event.key !== "Enter") return false;

  const sinceCompositionEnd = event.timeStamp - compositionEndAt;
  return (
    composing ||
    event.isComposing ||
    event.keyCode === 229 ||
    (sinceCompositionEnd >= 0 &&
      sinceCompositionEnd < IME_COMMIT_GRACE_MS)
  );
}
