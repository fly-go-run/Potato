import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const bridge = vi.hoisted(() => ({
  hasDesktopHostBridge: vi.fn(() => true),
  invokeDesktop: vi.fn(),
  listenDesktopEvent: vi.fn(),
}));
vi.mock("./desktop", () => bridge);

beforeEach(() => { vi.resetModules(); vi.clearAllMocks(); bridge.hasDesktopHostBridge.mockReturnValue(true); });
afterEach(() => { vi.unstubAllGlobals(); });

describe("native desktop transport", () => {
  it("sends document bytes to the Rust parser and preserves parser errors", async () => {
    const { nativeFetch } = await import("./nativeTransport");
    const form = new FormData();
    form.append("file", new File(["<script>not executable</script>"], "notes.html", { type: "text/html" }));
    bridge.invokeDesktop.mockResolvedValueOnce({ url: `data:text/plain;base64,${btoa("<script>not executable</script>")}` });
    const response = await nativeFetch("/api/console/upload", { method: "POST", body: form });
    expect((await response.json()).url).toBe(`data:text/plain;base64,${btoa("<script>not executable</script>")}`);
    const binary = new FormData();
    binary.append("file", new File([new Uint8Array([0, 255, 10])], "document.pdf"));
    bridge.invokeDesktop.mockRejectedValueOnce({ status: 415, message: "Unsupported document format" });
    expect((await nativeFetch("/api/console/upload", { method: "POST", body: binary })).status).toBe(415);
  });
  it("returns backup ZIP bytes instead of a JSON blob", async () => {
    bridge.invokeDesktop.mockResolvedValue({ native_binary: btoa("PK fixture"), mime: "application/zip" });
    const { nativeFetch } = await import("./nativeTransport");
    const response = await nativeFetch("/api/workspace/download", {});
    expect(response.headers.get("Content-Type")).toBe("application/zip");
    expect(await response.text()).toBe("PK fixture");
  });

  it("uploads a skill archive through native IPC without HTTP", async () => {
    bridge.invokeDesktop.mockResolvedValue({ name: "family" });
    const form = new FormData();
    form.append("file", new File(["PK fixture"], "family.zip", { type: "application/zip" }));
    const { nativeFetch } = await import("./nativeTransport");
    const response = await nativeFetch("/api/skills/upload", { method: "POST", body: form });
    expect(await response.json()).toEqual({ name: "family" });
    expect(bridge.invokeDesktop).toHaveBeenCalledWith("native_request", {
      method: "POST", path: "/api/skills/upload", body: { filename: "family.zip", base64: btoa("PK fixture") },
    });
  });
  it("detects native mode once and does not make HTTP requests", async () => {
    bridge.invokeDesktop.mockResolvedValue(true);
    const fetch = vi.fn(); vi.stubGlobal("fetch", fetch);
    const { detectNativeRuntime } = await import("./nativeTransport");
    expect(await detectNativeRuntime()).toBe(true);
    expect(await detectNativeRuntime()).toBe(true);
    expect(bridge.invokeDesktop).toHaveBeenCalledTimes(1);
    expect(fetch).not.toHaveBeenCalled();
  });

  it("falls back for older desktop shells", async () => {
    bridge.invokeDesktop.mockRejectedValue(new Error("unknown command"));
    const { detectNativeRuntime } = await import("./nativeTransport");
    expect(await detectNativeRuntime()).toBe(false);
  });

  it("preserves structured native API errors", async () => {
    bridge.invokeDesktop.mockRejectedValue({ status: 409, message: "Stop the running turn" });
    const { nativeFetch } = await import("./nativeTransport");
    const response = await nativeFetch("/api/chats/id", { method: "DELETE" });
    expect(response.status).toBe(409);
    expect(await response.json()).toEqual({ detail: "Stop the running turn" });
  });

  it("subscribes before starting, preserves event order, closes and releases listener", async () => {
    let handler: (frame: Record<string, unknown>) => void;
    const unlisten = vi.fn();
    bridge.listenDesktopEvent.mockImplementation(async (_event, callback) => { handler = callback; return unlisten; });
    bridge.invokeDesktop.mockImplementation(async (command) => {
      if (command === "native_chat_start") {
        handler({ object: "content", text: "你好" });
        handler({ object: "response", status: "completed" });
      }
    });
    const { nativeFetch } = await import("./nativeTransport");
    const response = await nativeFetch("/api/console/chat", { method: "POST", body: "{}" });
    expect(await response.text()).toBe('data: {"object":"content","text":"你好"}\n\ndata: {"object":"response","status":"completed"}\n\n');
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("cancels again when abort races with native command admission", async () => {
    const unlisten = vi.fn();
    bridge.listenDesktopEvent.mockResolvedValue(unlisten);
    let resolveStart: () => void;
    const start = new Promise<void>((resolve) => { resolveStart = resolve; });
    const abort = new AbortController();
    bridge.invokeDesktop.mockImplementation(async (command) => { if (command === "native_chat_start") { abort.abort(); await start; } });
    const { nativeFetch } = await import("./nativeTransport");
    const result = nativeFetch("/api/console/chat", { method: "POST", body: "{}", signal: abort.signal });
    await vi.waitFor(() => expect(abort.signal.aborted).toBe(true));
    resolveStart!();
    await expect(result).rejects.toMatchObject({ name: "AbortError" });
    expect(bridge.invokeDesktop.mock.calls.filter(([name]) => name === "native_chat_cancel")).toHaveLength(2);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("does not start a model request when listener registration fails", async () => {
    bridge.listenDesktopEvent.mockResolvedValue(null);
    const { nativeFetch } = await import("./nativeTransport");
    expect((await nativeFetch("/api/console/chat", { method: "POST", body: "{}" })).status).toBe(500);
    expect(bridge.invokeDesktop).not.toHaveBeenCalled();
  });
});
