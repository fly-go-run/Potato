import { describe, expect, it } from "vitest";
import { BOTTOM_THRESHOLD_PX, isAtBottom } from "./scroll";

describe("scroll bottom detection", () => {
  it("treats the threshold boundary as at the bottom", () => {
    expect(
      isAtBottom({
        scrollHeight: 1_000,
        scrollTop: 520,
        clientHeight: 400,
      }),
    ).toBe(true);
    expect(BOTTOM_THRESHOLD_PX).toBe(80);
  });

  it("stops following when the remaining distance exceeds the threshold", () => {
    expect(
      isAtBottom({
        scrollHeight: 1_000,
        scrollTop: 519,
        clientHeight: 400,
      }),
    ).toBe(false);
  });

  it("accepts a custom threshold", () => {
    expect(
      isAtBottom(
        { scrollHeight: 1_000, scrollTop: 550, clientHeight: 400 },
        40,
      ),
    ).toBe(false);
  });
});
