import { beforeEach, describe, expect, it, vi } from "vitest";
const bridge = vi.hoisted(() => ({ invokeDesktop: vi.fn(), listenDesktopEvent: vi.fn() }));
vi.mock("./desktop", () => bridge);
beforeEach(() => { vi.clearAllMocks(); });

describe("native Doubao voice bridge", () => {
  it("waits for pending PCM chunks before sending stop and resolves the final transcript", async () => {
    let handler: (frame: unknown) => void;
    const cleanup = vi.fn();
    bridge.listenDesktopEvent.mockImplementation(async (_event, callback) => { handler = callback; return cleanup; });
    let finishAudio: () => void;
    const audio = new Promise<void>((resolve) => { finishAudio = resolve; });
    bridge.invokeDesktop.mockImplementation(async (command, args) => {
      if (command === "native_voice_audio") await audio;
      if (command === "native_voice_end" && !args.cancel) handler({ type: "final", text: "测试语音" });
    });
    const handlers = { onPartial: vi.fn(), onFinal: vi.fn(), onError: vi.fn() };
    const { openNativeVoice } = await import("./nativeVoice");
    const voice = await openNativeVoice(handlers);
    voice.sendPcm(new Uint8Array([0, 0]).buffer);
    const final = voice.stop();
    await Promise.resolve();
    expect(bridge.invokeDesktop.mock.calls.some(([name]) => name === "native_voice_end")).toBe(false);
    finishAudio!();
    expect(await final).toBe("测试语音");
    expect(handlers.onFinal).toHaveBeenCalledWith("测试语音");
    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  it("cleans up when native speech admission fails", async () => {
    const cleanup = vi.fn(); bridge.listenDesktopEvent.mockResolvedValue(cleanup);
    bridge.invokeDesktop.mockRejectedValue({ message: "Missing speech credentials" });
    const { openNativeVoice } = await import("./nativeVoice");
    await expect(openNativeVoice({ onPartial: vi.fn(), onError: vi.fn() })).rejects.toThrow("Missing speech credentials");
    expect(cleanup).toHaveBeenCalledTimes(1);
  });
});
