import { describe, expect, it } from "vitest";
import {
  shouldShowLiveSignal,
  shouldShowProcessHeader,
} from "./processHeader";

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

  it("shows Worked for… on a settled turn that actually did work", () => {
    expect(
      shouldShowProcessHeader({
        ...base,
        elapsedMs: 8_000,
        settled: true,
        hasProcessWork: true,
      }),
    ).toBe(true);
    expect(
      shouldShowProcessHeader({
        ...base,
        elapsedMs: 8_000,
        settled: true,
        hasProcessWork: false,
      }),
    ).toBe(false);
  });
});

describe("shouldShowLiveSignal", () => {
  it("shows only while the turn is live and still empty", () => {
    expect(
      shouldShowLiveSignal({
        live: true,
        showHeader: false,
        hasVisiblePiece: false,
        hasVisibleToolFold: false,
      }),
    ).toBe(true);
    expect(
      shouldShowLiveSignal({
        live: true,
        showHeader: false,
        hasVisiblePiece: true,
        hasVisibleToolFold: false,
      }),
    ).toBe(false);
    expect(
      shouldShowLiveSignal({
        live: true,
        showHeader: false,
        hasVisiblePiece: false,
        hasVisibleToolFold: true,
      }),
    ).toBe(false);
    expect(
      shouldShowLiveSignal({
        live: true,
        showHeader: true,
        hasVisiblePiece: false,
        hasVisibleToolFold: false,
      }),
    ).toBe(false);
    expect(
      shouldShowLiveSignal({
        live: false,
        showHeader: false,
        hasVisiblePiece: false,
        hasVisibleToolFold: false,
      }),
    ).toBe(false);
  });
});
