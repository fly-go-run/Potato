import { describe, expect, it } from "vitest";
import { outputPreview, shellPresentation } from "./toolPresentation";

describe("tool result presentation", () => {
  it("does not turn empty stdout or legacy success text into an empty-directory claim", () => {
    const result = shellPresentation(JSON.stringify({command: 'find "$HOME/Desktop" -printf "%f" 2>/dev/null | sort'}), "Command executed successfully (no output).");
    expect(result).toMatchObject({ label: "tool.action.desktop", empty: true, hiddenErrors: true, preview: "" });
  });
  it("keeps failures visible as output and does not interpret arbitrary shell scripts", () => {
    expect(shellPresentation('{"command":"python task.py"}', "Command failed with exit code 1.").preview).toContain("exit code 1");
    expect(shellPresentation('{"command":"python task.py"}', "").label).toBe("tool.action.command");
  });
  it("bounds live logs by line and column while preserving their text", () => {
    expect(outputPreview("old\nfirst\nsecond\nlatest", 3)).toBe("first\nsecond\nlatest");
    expect(outputPreview("x".repeat(500)).length).toBe(240);
    expect(outputPreview("\u001b[31merror\u001b[0m")).toBe("error");
  });
  it("preserves stderr even when the command produced no stdout", () => {
    const result = shellPresentation('{"command":"ls"}', "Command exited with code 0 and produced no stdout.\n[stderr]\nwarning");
    expect(result.empty).toBe(false);
    expect(result.preview).toContain("warning");
  });
  it("accepts incomplete streaming arguments without throwing", () => {
    expect(shellPresentation('{"command":', "partial").preview).toBe("partial");
  });
});
