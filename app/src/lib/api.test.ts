import { afterEach, describe, expect, it, vi } from "vitest";
import { modelApi, providerReady, type ProviderInfo } from "./api";

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

describe("modelApi.configure", () => {
  it("uses the partial-update PUT endpoint directly", async () => {
    vi.stubGlobal("localStorage", {
      getItem: () => null,
      setItem: () => undefined,
      removeItem: () => undefined,
    });
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: "deepseek" }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await modelApi.configure("deepseek", { api_key: "sk-new" });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/models/deepseek/config",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({ api_key: "sk-new" }),
      }),
    );
  });
});

describe("providerReady", () => {
  const baseProvider = {
    id: "provider",
    name: "Provider",
    base_url: "https://example.com",
    api_key: "",
    chat_model: "OpenAIChatModel",
    models: [],
    extra_models: [],
    api_key_prefix: "",
    api_key_prefixes: [],
    is_local: false,
    freeze_url: false,
    require_api_key: true,
    is_custom: false,
  } satisfies ProviderInfo;

  it("accepts local and keyless providers, but not unconfigured API providers", () => {
    expect(providerReady(baseProvider)).toBe(false);
    expect(providerReady({ ...baseProvider, api_key: "sk-******" })).toBe(true);
    expect(providerReady({ ...baseProvider, require_api_key: false })).toBe(true);
    expect(providerReady({ ...baseProvider, is_local: true })).toBe(true);
  });
});
