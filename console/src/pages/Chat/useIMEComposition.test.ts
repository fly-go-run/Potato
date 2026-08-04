import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import type { Mock } from "vitest";
import { renderHook } from "@testing-library/react";
import { useIMEComposition, IME_COMMIT_GRACE_MS } from "./useIMEComposition";

/**
 * Regression tests for the WebKit/WKWebView IME event ordering: pressing
 * Enter to commit a pinyin composition fires compositionend *first*, then a
 * keydown with isComposing=false and a normal keyCode.  That keydown must be
 * suppressed so the raw letters land in the textarea instead of being sent.
 */
describe("useIMEComposition", () => {
  let textarea: HTMLTextAreaElement;
  let reachedSender: Mock<(event: KeyboardEvent) => void>;
  let unmount: () => void;

  const dispatchEnter = () => {
    const enter = new KeyboardEvent("keydown", {
      key: "Enter",
      bubbles: true,
      cancelable: true,
    });
    textarea.dispatchEvent(enter);
    return enter;
  };

  beforeEach(() => {
    ({ unmount } = renderHook(() => useIMEComposition(() => true)));
    textarea = document.createElement("textarea");
    document.body.appendChild(textarea);
    // Stands in for the Sender's own keydown/onPressEnter handler: if the
    // capture-phase guard suppresses the event, this must never fire.
    reachedSender = vi.fn<(event: KeyboardEvent) => void>();
    textarea.addEventListener("keydown", reachedSender);
  });

  afterEach(() => {
    unmount();
    textarea.remove();
  });

  const startComposition = () =>
    textarea.dispatchEvent(
      new CompositionEvent("compositionstart", { bubbles: true }),
    );
  const endComposition = () =>
    textarea.dispatchEvent(
      new CompositionEvent("compositionend", { bubbles: true }),
    );

  it("suppresses the committing Enter fired right after compositionend (WebKit ordering)", () => {
    startComposition();
    endComposition();

    // Same keystroke as the compositionend, so it arrives within the grace
    // window with isComposing=false and a normal keyCode.
    const enter = dispatchEnter();

    expect(reachedSender).not.toHaveBeenCalled();
    expect(enter.defaultPrevented).toBe(true);
  });

  it("lets the next Enter after the grace window submit normally", async () => {
    startComposition();
    endComposition();
    dispatchEnter();
    reachedSender.mockClear();

    await new Promise((r) => setTimeout(r, IME_COMMIT_GRACE_MS + 30));
    const enter = dispatchEnter();

    expect(reachedSender).toHaveBeenCalledTimes(1);
    expect(enter.defaultPrevented).toBe(false);
  });

  it("keeps suppressing while a new composition follows a previous one (stale-reset regression)", async () => {
    // Old implementation cleared the composing flag on a 50ms timer that was
    // never cancelled, so a commit followed quickly by more typing lost its
    // guard mid-composition.
    startComposition();
    endComposition();
    startComposition();

    await new Promise((r) => setTimeout(r, IME_COMMIT_GRACE_MS + 30));
    const enter = dispatchEnter();

    expect(reachedSender).not.toHaveBeenCalled();
    expect(enter.defaultPrevented).toBe(true);
  });

  it("does not interfere with Enter when no composition happened", () => {
    const enter = dispatchEnter();

    expect(reachedSender).toHaveBeenCalledTimes(1);
    expect(enter.defaultPrevented).toBe(false);
  });
});
