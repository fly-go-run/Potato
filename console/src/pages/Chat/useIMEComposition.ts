import { useCallback, useEffect, useRef } from "react";

/**
 * Grace window after compositionend during which an Enter keydown is treated
 * as the IME commit keystroke rather than a submit.  Safari/WKWebView fires
 * the committing Enter keydown *after* compositionend with isComposing
 * already false, so flag checks alone cannot catch it.  Keep this short so
 * fast typists who hit Space+Enter in quick succession are not blocked.
 */
export const IME_COMMIT_GRACE_MS = 50;

/** Handle IME composition events to prevent premature Enter key submission. */
export function useIMEComposition(isChatActive: () => boolean) {
  const isComposingRef = useRef(false);
  const compositionEndAtRef = useRef(Number.NEGATIVE_INFINITY);

  // keyCode 229 marks IME-processed keys in WebKit; the timeStamp comparison
  // (same clock as the recorded compositionend event, so immune to
  // main-thread jank between the two dispatches) covers Safari/WKWebView's
  // post-compositionend Enter keydown.
  const isImeEnter = useCallback(
    (e: KeyboardEvent) =>
      isComposingRef.current ||
      e.isComposing ||
      e.keyCode === 229 ||
      e.timeStamp - compositionEndAtRef.current < IME_COMMIT_GRACE_MS,
    [],
  );

  // For submit-time guards that have no keyboard event to inspect.
  const isImeRecentlyActive = useCallback(
    () =>
      isComposingRef.current ||
      performance.now() - compositionEndAtRef.current < IME_COMMIT_GRACE_MS,
    [],
  );

  useEffect(() => {
    const handleCompositionStart = () => {
      if (!isChatActive()) return;
      isComposingRef.current = true;
    };

    const handleCompositionEnd = (e: CompositionEvent) => {
      if (!isChatActive()) return;
      // Reset synchronously — a delayed reset can fire in the middle of the
      // *next* composition session and wrongly mark it as not composing.
      // The post-commit grace period is handled via the timestamp instead.
      isComposingRef.current = false;
      compositionEndAtRef.current = e.timeStamp;
    };

    const suppressImeEnter = (e: KeyboardEvent) => {
      if (!isChatActive()) return;
      const target = e.target as HTMLElement;
      if (target?.tagName === "TEXTAREA" && e.key === "Enter" && !e.shiftKey) {
        if (isImeEnter(e)) {
          e.stopPropagation();
          e.stopImmediatePropagation();
          e.preventDefault();
          return false;
        }
      }
    };

    document.addEventListener("compositionstart", handleCompositionStart, true);
    document.addEventListener("compositionend", handleCompositionEnd, true);
    // Listen on both keydown (Safari) and keypress (legacy) in capture phase.
    document.addEventListener("keydown", suppressImeEnter, true);
    document.addEventListener("keypress", suppressImeEnter, true);

    return () => {
      document.removeEventListener(
        "compositionstart",
        handleCompositionStart,
        true,
      );
      document.removeEventListener(
        "compositionend",
        handleCompositionEnd,
        true,
      );
      document.removeEventListener("keydown", suppressImeEnter, true);
      document.removeEventListener("keypress", suppressImeEnter, true);
    };
  }, [isChatActive, isImeEnter]);

  return { isComposingRef, isImeEnter, isImeRecentlyActive };
}
