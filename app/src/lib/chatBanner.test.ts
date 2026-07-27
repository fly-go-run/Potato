import { describe, expect, it } from "vitest";
import { getChatBanner } from "./chatBanner";

describe("chat banner branches", () => {
  it("uses a warning banner and backend text for rate limits", () => {
    const banner = getChatBanner("duplicate", {
      type: "rate_limited",
      error: "Free quota exhausted",
      alternatives: [
        {
          provider_id: "free-provider",
          provider_name: "Free Provider",
          model_id: "free-model",
          model_name: "Free Model",
        },
      ],
    });
    expect(banner).toMatchObject({
      tone: "warn",
      message: "Free quota exhausted",
    });
    expect(banner?.alternatives).toHaveLength(1);
  });

  it("uses danger for ordinary errors and nothing for a clean state", () => {
    expect(getChatBanner("Connection lost", null)).toEqual({
      tone: "danger",
      message: "Connection lost",
      alternatives: [],
    });
    expect(getChatBanner(null, null)).toBeNull();
  });
});
