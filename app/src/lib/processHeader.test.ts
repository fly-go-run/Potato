import { describe, expect, it } from "vitest";
import { shouldShowProcessHeader } from "./processHeader";

const base = {
  elapsedMs: 38_000,
  failed: 0,
  toolFoldCount: 1,
  foldWindow: 8,
};

describe("shouldShowProcessHeader", () => {
  it("hides the header on a short single-tool turn", () => {
    expect(shouldShowProcessHeader(base)).toBe(false);
    expect(shouldShowProcessHeader({ ...base, elapsedMs: null })).toBe(false);
    expect(shouldShowProcessHeader({ ...base, toolFoldCount: 0 })).toBe(false);
  });

  it("shows the header after a minute, on failure, or on overflow", () => {
    expect(
      shouldShowProcessHeader({ ...base, elapsedMs: 60_000 }),
    ).toBe(true);
    expect(shouldShowProcessHeader({ ...base, failed: 1 })).toBe(true);
    expect(
      shouldShowProcessHeader({ ...base, toolFoldCount: 9 }),
    ).toBe(true);
  });
});
