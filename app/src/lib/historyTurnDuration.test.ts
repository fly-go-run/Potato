import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { historyTurnDuration } from "./historyTurnDuration";
import { formatDuration } from "./messageTiming";

const fixture = JSON.parse(
  readFileSync(
    join(
      dirname(fileURLToPath(import.meta.url)),
      "../../fixtures/http/chat-history-tool-call.json",
    ),
    "utf8",
  ),
) as {
  messages: Array<{
    role: string;
    metadata?: { timestamp?: string } | null;
  }>;
};

function timestampOf(
  message: { metadata?: { timestamp?: string } | null } | undefined,
): unknown {
  return message?.metadata?.timestamp;
}

describe("historyTurnDuration", () => {
  it("formats the fixture user 21:17:48 → last assistant 21:17:53 as ~5s", () => {
    const user = fixture.messages.find((message) => message.role === "user");
    const lastAssistant = [...fixture.messages]
      .reverse()
      .find((message) => message.role === "assistant");
    const start = timestampOf(user);
    const end = timestampOf(lastAssistant);

    expect(start).toBe("2026-07-27T21:17:48.939256+08:00");
    expect(end).toBe("2026-07-27T21:17:53.713278+08:00");
    expect(historyTurnDuration(start, end)).toBe(
      formatDuration(
        Date.parse(end as string) - Date.parse(start as string),
      ),
    );
    expect(historyTurnDuration(start, end)).toBe("4.8s");
  });

  it("returns an empty string when end <= start", () => {
    const stamp = "2026-07-27T21:17:48.939256+08:00";
    expect(historyTurnDuration(stamp, stamp)).toBe("");
    expect(
      historyTurnDuration(stamp, "2026-07-27T21:17:47.000000+08:00"),
    ).toBe("");
  });

  it("returns an empty string when either timestamp cannot be parsed", () => {
    expect(historyTurnDuration(undefined, "2026-07-27T21:17:53.713278+08:00")).toBe(
      "",
    );
    expect(historyTurnDuration("2026-07-27T21:17:48.939256+08:00", null)).toBe(
      "",
    );
    expect(historyTurnDuration("not-a-date", "also-not-a-date")).toBe("");
  });
});
