import { describe, expect, it } from "vitest";
import { extractFirstBold } from "./reasoningTitle";

describe("extractFirstBold", () => {
  it("returns the first closed bold phrase", () => {
    expect(extractFirstBold("**Exploring codebase** then more")).toBe(
      "Exploring codebase",
    );
  });

  it("returns null when the opener is never closed", () => {
    expect(extractFirstBold("**Exploring codebase")).toBeNull();
    expect(extractFirstBold("still thinking")).toBeNull();
    expect(extractFirstBold("")).toBeNull();
  });

  it("keeps only the first of several closed phrases", () => {
    expect(
      extractFirstBold("**First look** then later **Second pass**"),
    ).toBe("First look");
  });

  it("extracts a closed CJK phrase", () => {
    expect(extractFirstBold("先 **探索代码库** 再动手")).toBe("探索代码库");
  });
});
