// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import ToolCardShell from "./ToolCardShell";

describe("ToolCardShell", () => {
  const content = {
    type: "tool_call" as const,
    id: "tool-1",
    name: "execute_shell_command",
    params: { command: "printf hello" },
    status: "done" as const,
    result: "hello",
  };

  it("keeps command details collapsed until the summary is clicked", () => {
    const { container } = render(
      <ToolCardShell content={content} icon={<span>⌘</span>} title="Ran command">
        <div>command output</div>
      </ToolCardShell>,
    );

    const details = container.querySelector("details");
    expect(details).not.toBeNull();
    expect(details).not.toHaveAttribute("open");

    fireEvent.click(screen.getByText("Ran command"));

    expect(details).toHaveAttribute("open");
  });

  it("shows a compact duration without opening the output", () => {
    const { container } = render(
      <ToolCardShell
        content={{ ...content, durationMs: 2000 }}
        icon={<span>⌘</span>}
        title="Ran command"
        showDuration
      >
        <div>command output</div>
      </ToolCardShell>,
    );

    expect(screen.getByText("2s")).toBeInTheDocument();
    expect(container.querySelector("details")).not.toHaveAttribute("open");
  });
});
