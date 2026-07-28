import { afterEach, describe, expect, it, vi } from "vitest";
import { modelApi } from "./api";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("modelApi.setActive", () => {
  it("updates the current agent scope instead of a masked global model", async () => {
    vi.stubGlobal("localStorage", {
      getItem: () => null,
      setItem: () => undefined,
      removeItem: () => undefined,
    });
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        new Response(JSON.stringify({ agent_id: "agent-current" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            active_llm: { provider_id: "openai", model: "gpt-5" },
            effective_max_input_length: 128000,
          }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      );
    vi.stubGlobal("fetch", fetchMock);

    await modelApi.setActive("openai", "gpt-5");

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "/api/workspace/language",
      expect.any(Object),
    );
    const [, init] = fetchMock.mock.calls[1] as [string, RequestInit];
    expect(JSON.parse(String(init.body))).toEqual({
      provider_id: "openai",
      model: "gpt-5",
      scope: "agent",
      agent_id: "agent-current",
    });
  });
});
