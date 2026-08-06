import { afterEach, describe, expect, it, vi } from "vitest";
import {
  modelApi,
  providerConfigured,
  providerReady,
  type ProviderInfo,
} from "./api";

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

describe("modelApi.createCustomProvider", () => {
  it("carries the selected wire protocol to the backend", async () => {
    vi.stubGlobal("localStorage", {
      getItem: () => null,
      setItem: () => undefined,
      removeItem: () => undefined,
    });
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: "my-gateway" }), {
        status: 201,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await modelApi.createCustomProvider({
      id: "my-gateway",
      name: "My Gateway",
      default_base_url: "http://127.0.0.1:8788/v1",
      chat_model: "OpenAIResponseModel",
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/models/custom-providers",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          id: "my-gateway",
          name: "My Gateway",
          default_base_url: "http://127.0.0.1:8788/v1",
          chat_model: "OpenAIResponseModel",
        }),
      }),
    );
  });
});

describe("modelApi.removeProvider", () => {
  it("deletes a custom provider and returns the refreshed provider list", async () => {
    vi.stubGlobal("localStorage", {
      getItem: () => null,
      setItem: () => undefined,
      removeItem: () => undefined,
    });
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify([]), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await modelApi.removeProvider("open-code/custom");

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/models/custom-providers/open-code%2Fcustom",
      expect.objectContaining({ method: "DELETE" }),
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
    expect(providerReady({ ...baseProvider, require_api_key: false })).toBe(
      true,
    );
    expect(providerReady({ ...baseProvider, is_local: true })).toBe(true);
  });
});

describe("providerConfigured", () => {
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

  it("only marks saved credentials or custom endpoints as configured", () => {
    expect(providerConfigured(baseProvider)).toBe(false);
    expect(
      providerConfigured({ ...baseProvider, require_api_key: false }),
    ).toBe(false);
    expect(providerConfigured({ ...baseProvider, api_key: "sk-******" })).toBe(
      true,
    );
    // 需要 key 的自定义供应商,没有 key(如刚被清除)不能算已配置
    expect(
      providerConfigured({
        ...baseProvider,
        is_custom: true,
        api_key: "",
      }),
    ).toBe(false);
    // 声明不需要 key 的自定义供应商,仅凭端点即算已配置
    expect(
      providerConfigured({
        ...baseProvider,
        is_custom: true,
        api_key: "",
        require_api_key: false,
      }),
    ).toBe(true);
    expect(
      providerConfigured({
        ...baseProvider,
        is_local: true,
        require_api_key: false,
      }),
    ).toBe(false);
  });
});
