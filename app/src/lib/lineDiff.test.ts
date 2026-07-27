import { describe, expect, it } from "vitest";
import { lineDiff } from "./lineDiff";

describe("lineDiff", () => {
  it("keeps common lines and marks replacements as remove plus add", () => {
    expect(lineDiff("alpha\nold\nomega", "alpha\nnew\nomega")).toEqual([
      { kind: "same", text: "alpha" },
      { kind: "remove", text: "old" },
      { kind: "add", text: "new" },
      { kind: "same", text: "omega" },
    ]);
  });

  it("handles pure additions, removals, and empty input", () => {
    expect(lineDiff("", "one\ntwo")).toEqual([
      { kind: "add", text: "one" },
      { kind: "add", text: "two" },
    ]);
    expect(lineDiff("one\ntwo", "")).toEqual([
      { kind: "remove", text: "one" },
      { kind: "remove", text: "two" },
    ]);
  });
});
