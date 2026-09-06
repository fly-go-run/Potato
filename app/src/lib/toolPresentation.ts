/** Presentation uses observed output, never an inferred successful task outcome. */
export function outputPreview(value: unknown, lines = 3, edge: "head" | "tail" = "tail"): string {
  const text = typeof value === "string" ? value : value == null ? "" : JSON.stringify(value, null, 2);
  const allLines = text.replace(/\x1b\[[0-9;]*[A-Za-z]/g, "").trim().split("\n");
  return (edge === "head" ? allLines.slice(0, lines) : allLines.slice(-lines)).map((line) => line.slice(0, 240)).join("\n");
}
export function shellPresentation(argumentsJson: string, output: unknown, running = false) {
  let command = argumentsJson;
  try { const args = JSON.parse(argumentsJson); if (typeof args.command === "string") command = args.command; } catch { /* Partial streamed arguments. */ }
  const text = typeof output === "string" ? output.trim() : "";
  const empty = !text || text === "Command executed successfully (no output)." || (text.startsWith("Command exited with code 0 and produced no stdout.") && !text.includes("[stderr]"));
  const hiddenErrors = /2\s*>\s*\/dev\/null/.test(command);
  const label: "tool.action.desktop" | "tool.action.list" | "tool.action.read" | "tool.action.command" = /^\s*(find|ls)\s/.test(command)
    ? (/\bDesktop\b/.test(command) ? "tool.action.desktop" : "tool.action.list")
    : /^\s*(cat|head|tail)\s/.test(command) ? "tool.action.read" : "tool.action.command";
  return { command, label, empty, hiddenErrors, preview: empty ? "" : outputPreview(output, 3, running ? "tail" : "head") };
}
