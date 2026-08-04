import { describe, expect, it } from "vitest";
import { IME_COMMIT_GRACE_MS, isImeCommitEnter } from "./ime";

const enterEvent = (
  overrides: Partial<KeyboardEvent> = {},
): Pick<KeyboardEvent, "key" | "isComposing" | "keyCode" | "timeStamp"> => ({
  key: "Enter",
  isComposing: false,
  keyCode: 13,
  timeStamp: 100,
  ...overrides,
});

describe("isImeCommitEnter", () => {
  it("blocks Enter while the IME is composing", () => {
    expect(isImeCommitEnter(enterEvent(), true, Number.NEGATIVE_INFINITY)).toBe(
      true,
    );
    expect(
      isImeCommitEnter(
        enterEvent({ isComposing: true }),
        false,
        Number.NEGATIVE_INFINITY,
      ),
    ).toBe(true);
  });

  it("blocks WebKit's Enter immediately after compositionend", () => {
    expect(isImeCommitEnter(enterEvent({ timeStamp: 120 }), false, 100)).toBe(
      true,
    );
    expect(isImeCommitEnter(enterEvent({ keyCode: 229 }), false, 0)).toBe(true);
  });

  it("allows the next Enter after the grace window", () => {
    expect(
      isImeCommitEnter(
        enterEvent({ timeStamp: 100 + IME_COMMIT_GRACE_MS }),
        false,
        100,
      ),
    ).toBe(false);
  });
});
