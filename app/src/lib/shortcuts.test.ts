import { describe, expect, it } from "vitest";
import {
  isPrimaryShortcut,
  shortcutLabel,
  shortcutModifier,
} from "./shortcuts";

describe("platform shortcuts", () => {
  it("uses Command on Apple platforms", () => {
    expect(shortcutModifier("MacIntel")).toBe("⌘");
    expect(shortcutLabel("K", "MacIntel")).toBe("⌘K");
    expect(
      isPrimaryShortcut({ metaKey: true, ctrlKey: false }, "MacIntel"),
    ).toBe(true);
    expect(
      isPrimaryShortcut({ metaKey: false, ctrlKey: true }, "MacIntel"),
    ).toBe(false);
  });

  it("uses Control elsewhere", () => {
    expect(shortcutModifier("Win32")).toBe("Ctrl");
    expect(shortcutLabel("K", "Win32")).toBe("Ctrl+K");
    expect(isPrimaryShortcut({ metaKey: false, ctrlKey: true }, "Win32")).toBe(
      true,
    );
  });
});
