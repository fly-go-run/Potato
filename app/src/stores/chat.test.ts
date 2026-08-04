import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  ApiError,
  chatApi,
  settingsApi,
  type ChatHistory,
  type ChatSpec,
} from "../lib/api";
import { initialConversationStreamState } from "../lib/stream";
import { useChatStore } from "./chat";

function memoryStorage(): Storage {
  const values = new Map<string, string>();
  return {
    get length() {
      return values.size;
    },
    clear: () => values.clear(),
    getItem: (key) => values.get(key) ?? null,
    key: (index) => [...values.keys()][index] ?? null,
    removeItem: (key) => {
      values.delete(key);
    },
    setItem: (key, value) => {
      values.set(key, value);
    },
  };
}

function chat(status: ChatSpec["status"]): ChatSpec {
  return {
    id: "chat-1",
    name: "Test",
    session_id: "session-1",
    user_id: "default",
    channel: "console",
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-01-01T00:00:00Z",
    status,
    pinned: false,
  };
}

function responseFrame(status: "in_progress" | "completed") {
  return {
    object: "response",
    id: "response-1",
    status,
    output: [],
    created_at: null,
    completed_at: status === "completed" ? "2026-01-01T00:00:01Z" : null,
    metadata: null,
    sequence_number: status === "completed" ? 2 : 1,
  };
}

function sseResponse(status: "in_progress" | "completed") {
  return new Response(`data: ${JSON.stringify(responseFrame(status))}\n\n`, {
    headers: { "Content-Type": "text/event-stream" },
  });
}

function controlledSseResponse() {
  const encoder = new TextEncoder();
  let finish!: () => void;
  const response = new Response(
    new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(
          encoder.encode(
            `data: ${JSON.stringify(responseFrame("in_progress"))}\n\n`,
          ),
        );
        finish = () => {
          controller.enqueue(
            encoder.encode(
              `data: ${JSON.stringify(responseFrame("completed"))}\n\n`,
            ),
          );
          controller.close();
        };
      },
    }),
    { headers: { "Content-Type": "text/event-stream" } },
  );
  return { response, finish: () => finish() };
}

function history(status: ChatHistory["status"]): ChatHistory {
  return {
    status,
    messages: [
      {
        object: "message",
        id: "user-1",
        type: "message",
        role: "user",
        content: [],
        status: "completed",
        metadata: null,
      },
    ],
  };
}

describe("chat stream interruption recovery", () => {
  beforeEach(() => {
    vi.stubGlobal("localStorage", memoryStorage());
    vi.stubGlobal("sessionStorage", memoryStorage());
    useChatStore.setState({
      chats: [],
      activeChatId: null,
      sessionId: "session-1",
      userId: "default",
      channel: "console",
      stream: initialConversationStreamState,
      isStreaming: false,
      isSubmitting: false,
      activeModel: {
        active_llm: { provider_id: "provider", model: "model" },
        effective_max_input_length: 8192,
      },
      error: null,
      project: null,
      pendingImages: [],
      pendingApprovals: [],
      requestController: null,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("reattaches the existing backend run instead of reposting the payload", async () => {
    vi.spyOn(chatApi, "stream")
      .mockResolvedValueOnce(sseResponse("in_progress"))
      .mockResolvedValueOnce(sseResponse("completed"));
    vi.spyOn(chatApi, "list")
      .mockResolvedValueOnce([chat("running")])
      .mockResolvedValue([chat("idle")]);
    vi.spyOn(chatApi, "get").mockResolvedValue(history("idle"));

    const accepted = await useChatStore
      .getState()
      .sendMessage("do the work", vi.fn());

    expect(accepted).toBe(true);
    expect(chatApi.stream).toHaveBeenCalledTimes(2);
    expect(vi.mocked(chatApi.stream).mock.calls[0]?.[0]).toMatchObject({
      input: [{ role: "user" }],
      session_id: "session-1",
    });
    expect(vi.mocked(chatApi.stream).mock.calls[1]?.[0]).toEqual({
      reconnect: true,
      session_id: "session-1",
      user_id: "default",
      channel: "console",
    });
    expect(useChatStore.getState()).toMatchObject({
      isStreaming: false,
      error: null,
      stream: { responseStatus: "completed" },
    });
  });

  it("keeps a repeatedly disconnected backend run busy and blocks a new turn", async () => {
    vi.spyOn(chatApi, "stream").mockImplementation(async () =>
      sseResponse("in_progress"),
    );
    vi.spyOn(chatApi, "list").mockResolvedValue([chat("running")]);
    vi.spyOn(chatApi, "get").mockResolvedValue(history("running"));

    await useChatStore.getState().sendMessage("first payload", vi.fn());

    expect(chatApi.stream).toHaveBeenCalledTimes(2);
    expect(useChatStore.getState()).toMatchObject({
      isStreaming: true,
      stream: { responseStatus: "in_progress" },
    });
    expect(useChatStore.getState().error).toContain("响应流提前断开");

    const accepted = await useChatStore
      .getState()
      .sendMessage("must not be dropped", vi.fn());

    expect(accepted).toBe(false);
    expect(chatApi.stream).toHaveBeenCalledTimes(2);
  });

  it("rejects a new payload when an interrupted response is still unfinished", async () => {
    const streamSpy = vi.spyOn(chatApi, "stream");
    useChatStore.setState({
      isStreaming: false,
      stream: {
        ...initialConversationStreamState,
        responseStatus: "in_progress",
      },
    });

    const accepted = await useChatStore
      .getState()
      .sendMessage("must stay local", vi.fn());

    expect(accepted).toBe(false);
    expect(streamSpy).not.toHaveBeenCalled();
    expect(useChatStore.getState().error).toContain("上一条消息仍在后台运行");
  });

  it("refreshes stale chat status after a conflict and preserves the rejected attachment", async () => {
    const revokeObjectURL = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(() => {});
    vi.spyOn(settingsApi, "uploadLimit").mockResolvedValue({
      upload_max_size_mb: 10,
    });
    vi.spyOn(chatApi, "upload").mockResolvedValue({
      url: "/api/files/preview/uploaded.png",
      file_name: "uploaded.png",
      size: 5,
    });
    vi.spyOn(chatApi, "stream")
      .mockRejectedValueOnce(
        new ApiError("A response is already running for this chat.", 409),
      )
      .mockResolvedValueOnce(sseResponse("completed"));
    const listSpy = vi
      .spyOn(chatApi, "list")
      .mockResolvedValueOnce([chat("running")])
      .mockResolvedValue([chat("idle")]);
    vi.spyOn(chatApi, "get").mockResolvedValue(history("idle"));
    const pendingImage = {
      id: "preview-conflict",
      file: new File(["image"], "conflict.png", { type: "image/png" }),
      previewUrl: "blob:preview-conflict",
    };
    useChatStore.setState({
      chats: [chat("idle")],
      activeChatId: "chat-1",
      pendingImages: [pendingImage],
    });

    const accepted = await useChatStore
      .getState()
      .sendMessage("keep this draft", vi.fn());

    expect(accepted).toBe(false);
    expect(listSpy).toHaveBeenCalled();
    expect(chatApi.stream).toHaveBeenCalledTimes(2);
    expect(vi.mocked(chatApi.stream).mock.calls[1]?.[0]).toEqual({
      reconnect: true,
      session_id: "session-1",
      user_id: "default",
      channel: "console",
    });
    expect(useChatStore.getState().pendingImages).toEqual([pendingImage]);
    expect(revokeObjectURL).not.toHaveBeenCalled();
    expect(useChatStore.getState().error).toBeNull();
  });

  it("restores the previous stream when a rejected request is no longer running", async () => {
    vi.spyOn(chatApi, "stream").mockRejectedValue(
      new ApiError("A response is already running for this chat.", 409),
    );
    vi.spyOn(chatApi, "list").mockResolvedValue([chat("idle")]);
    const previousMessage = {
      id: "previous-message",
      type: "message" as const,
      role: "assistant" as const,
      status: "completed" as const,
      content: [],
      metadata: null,
    };
    useChatStore.setState({
      chats: [chat("idle")],
      activeChatId: "chat-1",
      stream: {
        ...initialConversationStreamState,
        responseStatus: "completed",
        messages: [previousMessage],
      },
    });

    const accepted = await useChatStore
      .getState()
      .sendMessage("retry me", vi.fn());

    expect(accepted).toBe(false);
    expect(chatApi.stream).toHaveBeenCalledTimes(1);
    expect(useChatStore.getState()).toMatchObject({
      isStreaming: false,
      error: "A response is already running for this chat.",
      stream: {
        responseStatus: "failed",
        messages: [previousMessage],
      },
    });
  });

  it("does not restore an old chat after the conflict refresh is aborted", async () => {
    let resolveChats!: (chats: ChatSpec[]) => void;
    const delayedChats = new Promise<ChatSpec[]>((resolve) => {
      resolveChats = resolve;
    });
    const streamSpy = vi
      .spyOn(chatApi, "stream")
      .mockRejectedValue(
        new ApiError("A response is already running for this chat.", 409),
      );
    const listSpy = vi
      .spyOn(chatApi, "list")
      .mockImplementationOnce(() => delayedChats)
      .mockResolvedValue([]);
    useChatStore.setState({
      chats: [chat("idle")],
      activeChatId: "chat-1",
      stream: {
        ...initialConversationStreamState,
        responseStatus: "completed",
        messages: [
          {
            id: "old-chat-message",
            type: "message",
            role: "assistant",
            status: "completed",
            content: [],
            metadata: null,
          },
        ],
      },
    });

    const sending = useChatStore
      .getState()
      .sendMessage("do not resurrect this", vi.fn());
    await vi.waitFor(() => expect(listSpy).toHaveBeenCalledTimes(1));

    useChatStore.getState().newChat();
    resolveChats([chat("running")]);

    await expect(sending).resolves.toBe(false);
    expect(streamSpy).toHaveBeenCalledTimes(1);
    expect(useChatStore.getState()).toMatchObject({
      activeChatId: null,
      isStreaming: false,
      error: null,
      stream: { responseStatus: "idle", messages: [] },
    });
  });

  it("clears submitted attachments as soon as the request is accepted", async () => {
    const revokeObjectURL = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(() => {});
    vi.spyOn(settingsApi, "uploadLimit").mockResolvedValue({
      upload_max_size_mb: 10,
    });
    vi.spyOn(chatApi, "upload").mockResolvedValue({
      url: "/api/files/preview/uploaded.png",
      file_name: "uploaded.png",
      size: 5,
    });
    const controlled = controlledSseResponse();
    vi.spyOn(chatApi, "stream").mockResolvedValue(controlled.response);
    vi.spyOn(chatApi, "list").mockResolvedValue([]);
    const pendingImage = {
      id: "preview-accepted",
      file: new File(["image"], "accepted.png", { type: "image/png" }),
      previewUrl: "blob:preview-accepted",
    };
    useChatStore.setState({ pendingImages: [pendingImage] });

    const sending = useChatStore
      .getState()
      .sendMessage("describe this image", vi.fn());

    await vi.waitFor(() => {
      expect(useChatStore.getState()).toMatchObject({
        isStreaming: true,
        pendingImages: [],
      });
    });
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:preview-accepted");

    controlled.finish();
    await expect(sending).resolves.toBe(true);
  });

  it("can send again after immediately stopping a new chat", async () => {
    const revokeObjectURL = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(() => {});
    vi.spyOn(chatApi, "stream")
      .mockImplementationOnce(
        (_payload, signal) =>
          new Promise<Response>((_, reject) => {
            signal?.addEventListener("abort", () =>
              reject(new DOMException("Aborted", "AbortError")),
            );
          }),
      )
      .mockResolvedValueOnce(sseResponse("completed"));
    vi.spyOn(chatApi, "list").mockResolvedValue([]);

    const firstSend = useChatStore
      .getState()
      .sendMessage("first payload", vi.fn());
    await vi.waitFor(() => {
      expect(useChatStore.getState().stream.responseStatus).toBe("created");
    });
    useChatStore.setState({
      pendingImages: [
        {
          id: "preview-1",
          file: new File(["image"], "preview.png", { type: "image/png" }),
          previewUrl: "blob:preview-1",
        },
      ],
    });

    await useChatStore.getState().stop();
    await firstSend;

    expect(useChatStore.getState()).toMatchObject({
      isStreaming: false,
      isSubmitting: false,
      pendingImages: [],
      pendingApprovals: [],
      requestController: null,
      stream: { responseStatus: "cancelled" },
    });
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:preview-1");

    const accepted = await useChatStore
      .getState()
      .sendMessage("second payload", vi.fn());

    expect(accepted).toBe(true);
    expect(chatApi.stream).toHaveBeenCalledTimes(2);
  });
});
