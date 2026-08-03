import { create } from "zustand";
import type { NavigateFunction } from "react-router-dom";
import {
  ApiError,
  approvalApi,
  chatApi,
  modelApi,
  settingsApi,
  workspaceApi,
  type ActiveModelInfo,
  type ChatHistory,
  type ChatSpec,
} from "../lib/api";
import {
  filterApprovalsForSession,
  type PendingApproval,
} from "../lib/approvals";
import { t } from "../lib/i18n";
import { sortChats } from "../lib/chats";
import {
  hasSessionProjectRecord,
  loadLastProject,
  loadSessionProject,
  saveSessionProject,
  type ProjectBinding,
} from "../lib/projects";
import type {
  ContentBlock,
  FreeModelAlternative,
  MessageFrame,
  MessageKind,
  Role,
  RunStatus,
  TurnUsageFrame,
} from "../lib/protocol/types";
import {
  initialConversationStreamState,
  initialSseParserState,
  isUnfinishedResponse,
  isUnexpectedStreamEof,
  parseSseBytes,
  reduceStreamFrame,
  type ConversationStreamState,
  type StreamMessage,
} from "../lib/stream";
import {
  buildOutboundContent,
  findOversizedFile,
  type OutboundContentBlock,
  type UploadedAttachment,
} from "../lib/uploads";

export type ApprovalLevel = "STRICT" | "SMART" | "AUTO" | "OFF";

export interface PendingImage {
  id: string;
  file: File;
  previewUrl: string | null;
}

interface ChatStore {
  chats: ChatSpec[];
  chatsLoading: boolean;
  activeChatId: string | null;
  sessionId: string;
  userId: string;
  channel: string;
  stream: ConversationStreamState;
  isStreaming: boolean;
  isSubmitting: boolean;
  historyLoading: boolean;
  activeModel: ActiveModelInfo | null;
  modelLoading: boolean;
  error: string | null;
  approvalLevel: ApprovalLevel;
  project: ProjectBinding | null;
  pendingImages: PendingImage[];
  pendingApprovals: PendingApproval[];
  composerDraft: string | null;
  requestController: AbortController | null;

  initialize: () => Promise<void>;
  refreshChats: () => Promise<ChatSpec[]>;
  loadActiveModel: () => Promise<void>;
  loadApprovalLevel: () => Promise<void>;
  newChat: () => void;
  openChat: (chatId: string) => Promise<void>;
  sendMessage: (text: string, navigate: NavigateFunction) => Promise<boolean>;
  stop: () => Promise<void>;
  reconnect: (chat: ChatSpec) => Promise<void>;
  renameChat: (chatId: string, name: string) => Promise<void>;
  togglePinned: (chatId: string) => Promise<void>;
  deleteChat: (chatId: string) => Promise<void>;
  setApprovalLevel: (level: ApprovalLevel) => void;
  setComposerDraft: (text: string | null) => void;
  setProject: (project: ProjectBinding | null) => void;
  addImages: (files: File[]) => void;
  removeImage: (id: string) => void;
  pollApprovals: (signal?: AbortSignal) => Promise<void>;
  actOnApproval: (
    requestId: string,
    action: "approve" | "deny",
    scope?: "exact" | "similar",
  ) => Promise<void>;
  switchRateLimitedModel: (
    alternative: FreeModelAlternative,
  ) => Promise<boolean>;
  clearError: () => void;
}

const DEFAULT_USER_ID = "default";
const DEFAULT_CHANNEL = "console";
const PENDING_SESSION_KEY = "qwenpaw_pending_chat_session";

function createSessionId() {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 11)}`;
}

function initialProject() {
  return typeof globalThis.localStorage?.getItem === "function"
    ? loadLastProject()
    : null;
}

const initialSessionId = createSessionId();

export const useChatStore = create<ChatStore>((set, get) => ({
  chats: [],
  chatsLoading: false,
  activeChatId: null,
  sessionId: initialSessionId,
  userId: DEFAULT_USER_ID,
  channel: DEFAULT_CHANNEL,
  stream: initialConversationStreamState,
  isStreaming: false,
  isSubmitting: false,
  historyLoading: false,
  activeModel: null,
  modelLoading: false,
  error: null,
  approvalLevel: "AUTO",
  project: initialProject(),
  pendingImages: [],
  pendingApprovals: [],
  composerDraft: null,
  requestController: null,

  initialize: async () => {
    const [initialChats] = await Promise.all([
      get().refreshChats(),
      get().loadActiveModel(),
      get().loadApprovalLevel(),
    ]);
    const pendingSessionId = sessionStorage.getItem(PENDING_SESSION_KEY);
    if (
      pendingSessionId &&
      (!window.location.hash || window.location.hash === "#/")
    ) {
      let chats = initialChats;
      let pendingChat = chats.find(
        (chat) => chat.session_id === pendingSessionId,
      );
      if (!pendingChat) {
        await delay(300);
        chats = await get().refreshChats();
        pendingChat = chats.find(
          (chat) => chat.session_id === pendingSessionId,
        );
      }
      if (pendingChat) {
        sessionStorage.removeItem(PENDING_SESSION_KEY);
        window.location.hash = `#/chat/${pendingChat.id}`;
      }
    }
  },

  refreshChats: async () => {
    set({ chatsLoading: true });
    try {
      const chats = sortChats(await chatApi.list());
      set({ chats, chatsLoading: false });
      return chats;
    } catch (error) {
      set({ chatsLoading: false, error: readableError(error) });
      return [];
    }
  },

  loadActiveModel: async () => {
    set({ modelLoading: true });
    try {
      set({ activeModel: await modelApi.active(), modelLoading: false });
    } catch (error) {
      set({ modelLoading: false, error: readableError(error) });
    }
  },

  loadApprovalLevel: async () => {
    try {
      const config = await workspaceApi.runningConfig();
      const level = config.approval_level?.toUpperCase();
      if (isApprovalLevel(level)) set({ approvalLevel: level });
    } catch {
      // AUTO remains the contract-compatible fallback.
    }
  },

  newChat: () => {
    get().requestController?.abort();
    sessionStorage.removeItem(PENDING_SESSION_KEY);
    revokePreviews(get().pendingImages);
    const sessionId = createSessionId();
    const project = loadLastProject();
    saveSessionProject(sessionId, project);
    set({
      activeChatId: null,
      sessionId,
      userId: DEFAULT_USER_ID,
      channel: DEFAULT_CHANNEL,
      stream: initialConversationStreamState,
      isStreaming: false,
      isSubmitting: false,
      historyLoading: false,
      error: null,
      project,
      pendingImages: [],
      pendingApprovals: [],
      requestController: null,
    });
  },

  openChat: async (chatId) => {
    if (
      get().activeChatId === chatId &&
      !get().historyLoading &&
      get().stream.messages.length > 0
    ) {
      return;
    }
    get().requestController?.abort();
    const controller = new AbortController();
    let chat = get().chats.find((item) => item.id === chatId);
    set({
      activeChatId: chatId,
      sessionId: chat?.session_id ?? "",
      userId: chat?.user_id ?? DEFAULT_USER_ID,
      channel: chat?.channel ?? DEFAULT_CHANNEL,
      historyLoading: true,
      isStreaming: false,
      isSubmitting: false,
      error: null,
      project:
        chat && hasSessionProjectRecord(chat.session_id)
          ? loadSessionProject(chat.session_id)
          : null,
      pendingApprovals: [],
      requestController: controller,
    });
    if (!chat) {
      const chats = await get().refreshChats();
      if (controller.signal.aborted || get().activeChatId !== chatId) return;
      chat = chats.find((item) => item.id === chatId);
    }
    if (!chat) {
      set({
        error: t("chat.notFound"),
        historyLoading: false,
        requestController: null,
      });
      return;
    }
    set({
      sessionId: chat.session_id,
      userId: chat.user_id,
      channel: chat.channel,
      project: hasSessionProjectRecord(chat.session_id)
        ? loadSessionProject(chat.session_id)
        : null,
    });

    try {
      const history = await chatApi.get(chatId, controller.signal);
      if (controller.signal.aborted || get().activeChatId !== chatId) return;
      const messages = historyMessages(history);
      const turnUsage = historyTurnUsage(history, chat.session_id);
      set({
        stream: {
          ...initialConversationStreamState,
          messages,
          turnUsage,
          responseStatus: history.status === "running" ? "in_progress" : "idle",
        },
        historyLoading: false,
      });
      const currentChat =
        get().chats.find((item) => item.id === chatId) ?? chat;
      const pendingSessionId = sessionStorage.getItem(PENDING_SESSION_KEY);
      if (
        currentChat &&
        (history.status === "running" ||
          currentChat.status === "running" ||
          pendingSessionId === currentChat.session_id)
      ) {
        await get().reconnect({ ...currentChat, status: "running" });
      } else if (get().requestController === controller) {
        set({ requestController: null });
      }
    } catch (error) {
      if (!isAbort(error) && get().activeChatId === chatId) {
        set({
          historyLoading: false,
          error: readableError(error),
          requestController: null,
        });
      }
    }
  },

  sendMessage: async (rawText, navigate) => {
    const text = rawText.trim();
    const model = get().activeModel?.active_llm;
    if (!model) {
      set({ error: t("chat.modelRequired") });
      return false;
    }
    if (
      (!text && get().pendingImages.length === 0) ||
      get().isStreaming ||
      get().isSubmitting
    ) {
      return false;
    }
    if (isUnfinishedResponse(get().stream.responseStatus)) {
      const message = t("stream.turnStillRunning");
      set((state) => ({
        error: message,
        stream: { ...state.stream, error: message },
      }));
      return false;
    }

    set({ isSubmitting: true, error: null });
    let uploadedAttachments: UploadedAttachment[];
    try {
      uploadedAttachments = await uploadPendingFiles(
        get().pendingImages.map((attachment) => attachment.file),
      );
    } catch (error) {
      set({ isSubmitting: false, error: readableError(error) });
      return false;
    }
    if (!get().isSubmitting) return false;

    const outboundContent = buildOutboundContent(text, uploadedAttachments);
    const localMessage = userMessage(outboundContent);
    const controller = new AbortController();
    const baseStream: ConversationStreamState = {
      ...get().stream,
      responseId: null,
      responseStatus: "created",
      messages: [...get().stream.messages, localMessage],
      rateLimited: null,
      error: null,
      lastSequenceNumber: 0,
    };
    const sessionId = get().sessionId || createSessionId();
    saveSessionProject(sessionId, get().project);
    sessionStorage.setItem(PENDING_SESSION_KEY, sessionId);
    set({
      stream: baseStream,
      sessionId,
      isStreaming: true,
      isSubmitting: false,
      error: null,
      pendingApprovals: [],
      requestController: controller,
    });

    try {
      const response = await chatApi.stream(
        {
          input: [
            {
              role: "user",
              content: outboundContent,
            },
          ],
          session_id: sessionId,
          user_id: get().userId,
          channel: get().channel,
          stream: true,
          request_context: {
            approval_level: get().approvalLevel,
            ...(get().project
              ? { "qwenpaw.coding_project_dir": get().project?.path }
              : {}),
          },
        },
        controller.signal,
      );

      const chats = await get().refreshChats();
      const created = chats.find((chat) => chat.session_id === sessionId);
      if (created && get().sessionId === sessionId) {
        set({ activeChatId: created.id });
        navigate(`/chat/${created.id}`, { replace: true });
      }
      await consumeResponse(response, controller, set, get);
    } catch (error) {
      if (!isAbort(error)) {
        const message = readableError(error);
        const knownChat =
          get().chats.find((chat) => chat.session_id === sessionId) ??
          (await get().refreshChats()).find(
            (chat) => chat.session_id === sessionId,
          );
        if (knownChat?.status === "running") {
          await get().reconnect(knownChat);
        } else {
          set((state) => ({
            stream: {
              ...state.stream,
              responseStatus:
                error instanceof ApiError
                  ? "failed"
                  : state.stream.responseStatus,
              error: message,
            },
            error: message,
          }));
        }
      }
    } finally {
      if (get().requestController === controller) {
        if (
          sessionStorage.getItem(PENDING_SESSION_KEY) === sessionId &&
          isTerminalStatus(get().stream.responseStatus)
        ) {
          sessionStorage.removeItem(PENDING_SESSION_KEY);
        }
        revokePreviews(get().pendingImages);
        set({
          isStreaming: false,
          isSubmitting: false,
          requestController: null,
          pendingImages: [],
          pendingApprovals: [],
        });
      }
      await get().refreshChats();
    }
    return true;
  },

  stop: async () => {
    const { activeChatId, requestController, pendingImages, sessionId } = get();
    if (!activeChatId) {
      requestController?.abort();
      revokePreviews(pendingImages);
      if (sessionStorage.getItem(PENDING_SESSION_KEY) === sessionId) {
        sessionStorage.removeItem(PENDING_SESSION_KEY);
      }
      set((state) => ({
        isStreaming: false,
        isSubmitting: false,
        requestController: null,
        pendingImages: [],
        pendingApprovals: [],
        stream: { ...state.stream, responseStatus: "cancelled" },
      }));
      return;
    }
    try {
      await chatApi.stop(activeChatId);
      requestController?.abort();
      revokePreviews(get().pendingImages);
      set((state) => ({
        isStreaming: false,
        isSubmitting: false,
        requestController: null,
        pendingImages: [],
        pendingApprovals: [],
        stream: { ...state.stream, responseStatus: "cancelled" },
      }));
      await get().refreshChats();
    } catch (error) {
      set({ error: readableError(error) });
    }
  },

  reconnect: async (chat) => {
    get().requestController?.abort();
    const controller = new AbortController();
    set({
      activeChatId: chat.id,
      sessionId: chat.session_id,
      userId: chat.user_id,
      channel: chat.channel,
      isStreaming: true,
      isSubmitting: false,
      pendingApprovals: [],
      requestController: controller,
      stream: {
        ...get().stream,
        responseStatus: "in_progress",
        lastSequenceNumber: 0,
        error: null,
      },
    });
    let backendStillRunning = false;
    try {
      const response = await chatApi.stream(
        {
          reconnect: true,
          session_id: chat.session_id,
          user_id: chat.user_id,
          channel: chat.channel,
        },
        controller.signal,
      );
      await consumeResponse(response, controller, set, get);
    } catch (error) {
      if (!isAbort(error)) set({ error: readableError(error) });
    } finally {
      try {
        const history = await chatApi.get(chat.id);
        backendStillRunning = history.status === "running";
        if (get().activeChatId === chat.id) {
          set((state) => ({
            stream: {
              ...state.stream,
              messages: historyMessages(history),
              turnUsage: historyTurnUsage(history, chat.session_id),
              responseStatus:
                history.status === "running" ? "in_progress" : "completed",
            },
          }));
        }
        if (
          history.status !== "running" &&
          sessionStorage.getItem(PENDING_SESSION_KEY) === chat.session_id
        ) {
          sessionStorage.removeItem(PENDING_SESSION_KEY);
        }
      } catch (error) {
        if (!isAbort(error)) set({ error: readableError(error) });
      }
      if (get().requestController === controller) {
        set({
          isStreaming: backendStillRunning,
          isSubmitting: false,
          pendingApprovals: [],
          requestController: null,
        });
      }
      await get().refreshChats();
    }
  },

  renameChat: async (chatId, name) => {
    const trimmed = name.trim();
    if (!trimmed) return;
    try {
      const updated = await chatApi.update(chatId, { name: trimmed });
      set((state) => ({
        chats: sortChats(
          state.chats.map((chat) => (chat.id === chatId ? updated : chat)),
        ),
      }));
    } catch (error) {
      set({ error: readableError(error) });
    }
  },

  togglePinned: async (chatId) => {
    const chat = get().chats.find((item) => item.id === chatId);
    if (!chat) return;
    try {
      const updated = await chatApi.update(chatId, { pinned: !chat.pinned });
      set((state) => ({
        chats: sortChats(
          state.chats.map((item) => (item.id === chatId ? updated : item)),
        ),
      }));
    } catch (error) {
      set({ error: readableError(error) });
    }
  },

  deleteChat: async (chatId) => {
    try {
      await chatApi.delete(chatId);
      if (get().activeChatId === chatId) {
        get().newChat();
        window.location.hash = "#/";
      }
      set((state) => ({
        chats: state.chats.filter((chat) => chat.id !== chatId),
      }));
    } catch (error) {
      set({ error: readableError(error) });
    }
  },

  setApprovalLevel: (approvalLevel) => set({ approvalLevel }),
  setComposerDraft: (composerDraft) => set({ composerDraft }),

  setProject: (project) => {
    const sessionId = get().sessionId;
    if (sessionId) saveSessionProject(sessionId, project);
    set({ project });
  },

  addImages: (files) =>
    set((state) => ({
      pendingImages: [
        ...state.pendingImages,
        ...files.map((file) => ({
          id: `${file.name}-${file.lastModified}-${Math.random()}`,
          file,
          previewUrl: file.type.startsWith("image/")
            ? URL.createObjectURL(file)
            : null,
        })),
      ],
    })),

  removeImage: (id) =>
    set((state) => {
      const target = state.pendingImages.find((image) => image.id === id);
      if (target?.previewUrl) URL.revokeObjectURL(target.previewUrl);
      return {
        pendingImages: state.pendingImages.filter((image) => image.id !== id),
      };
    }),

  pollApprovals: async (signal) => {
    const sessionId = get().sessionId;
    if (!get().isStreaming || !sessionId) return;
    try {
      const response = await chatApi.pushMessages(sessionId, signal);
      if (
        !signal?.aborted &&
        get().isStreaming &&
        get().sessionId === sessionId
      ) {
        set({
          pendingApprovals: filterApprovalsForSession(
            response.pending_approvals,
            sessionId,
          ),
        });
      }
    } catch (error) {
      if (!isAbort(error)) {
        // Polling is best-effort; the next 2.5s tick retries automatically.
      }
    }
  },

  actOnApproval: async (requestId, action, scope = "exact") => {
    const approval = get().pendingApprovals.find(
      (item) => item.request_id === requestId,
    );
    if (!approval) return;
    try {
      await approvalApi.act(action, approval, scope);
      set((state) => ({
        pendingApprovals: state.pendingApprovals.filter(
          (item) => item.request_id !== requestId,
        ),
      }));
    } catch (error) {
      if (error instanceof ApiError && error.status === 404) {
        set((state) => ({
          pendingApprovals: state.pendingApprovals.filter(
            (item) => item.request_id !== requestId,
          ),
        }));
        return;
      }
      set({ error: readableError(error) });
    }
  },

  switchRateLimitedModel: async (alternative) => {
    try {
      await modelApi.setActive(alternative.provider_id, alternative.model_id);
      const activeModel = await modelApi.active();
      set((state) => ({
        activeModel,
        error: null,
        stream: {
          ...state.stream,
          error: null,
          rateLimited: null,
        },
      }));
      return true;
    } catch (error) {
      const message = readableError(error);
      set((state) => ({
        error: message,
        stream: {
          ...state.stream,
          error: message,
          rateLimited: null,
        },
      }));
      return false;
    }
  },

  clearError: () =>
    set((state) => ({
      error: null,
      stream: { ...state.stream, error: null, rateLimited: null },
    })),
}));

async function consumeResponse(
  response: Response,
  controller: AbortController,
  set: Parameters<typeof useChatStore.setState>[0] extends never
    ? never
    : typeof useChatStore.setState,
  get: typeof useChatStore.getState,
) {
  if (!response.body) throw new Error(t("stream.unreadable"));
  const reader = response.body.getReader();
  let parser = initialSseParserState;

  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    if (controller.signal.aborted) break;
    const parsed = parseSseBytes(value, parser);
    parser = parsed.state;
    for (const frame of parsed.frames) {
      const next = reduceStreamFrame(get().stream, frame);
      set({
        stream: next,
        error: next.error,
      });
    }
  }
  if (parser.errors.length > 0) {
    throw new Error(
      t("stream.parseFailed", { message: parser.errors[0] ?? "" }),
    );
  }
  if (
    isUnexpectedStreamEof(
      get().stream.responseStatus,
      controller.signal.aborted,
    )
  ) {
    throw new Error(t("stream.disconnected"));
  }
}

function historyMessages(history: ChatHistory): StreamMessage[] {
  if (!Array.isArray(history.messages)) return [];
  return history.messages
    .map(normalizeHistoryMessage)
    .filter((message): message is StreamMessage => message !== null);
}

function historyTurnUsage(
  history: ChatHistory,
  sessionId: string,
): TurnUsageFrame | null {
  for (let index = history.messages.length - 1; index >= 0; index -= 1) {
    const value = history.messages[index];
    if (!value || typeof value !== "object") continue;
    const metadata = (value as { metadata?: unknown }).metadata;
    if (!metadata || typeof metadata !== "object") continue;
    const nested = (metadata as { metadata?: unknown }).metadata;
    if (!nested || typeof nested !== "object") continue;
    const snapshot = (nested as { qwenpaw_turn_usage?: unknown })
      .qwenpaw_turn_usage;
    if (!snapshot || typeof snapshot !== "object") continue;
    const usage = (snapshot as { usage?: unknown }).usage;
    const contextUsage = (snapshot as { context_usage?: unknown })
      .context_usage;
    return {
      type: "turn_usage",
      session_id: sessionId,
      usage:
        usage && typeof usage === "object"
          ? (usage as Record<string, unknown>)
          : null,
      context_usage:
        contextUsage && typeof contextUsage === "object"
          ? (contextUsage as TurnUsageFrame["context_usage"])
          : null,
    };
  }
  return null;
}

function normalizeHistoryMessage(value: unknown): StreamMessage | null {
  if (!value || typeof value !== "object") return null;
  const frame = value as Partial<MessageFrame>;
  if (!frame.id || !frame.type) return null;
  return {
    id: frame.id,
    type: frame.type as MessageKind,
    role: (frame.role ?? null) as Role | null,
    status: (frame.status ?? "completed") as RunStatus,
    content: Array.isArray(frame.content)
      ? (frame.content.filter(isContentBlock) as ContentBlock[])
      : [],
    metadata: frame.metadata ?? null,
    name: frame.name,
    usage: frame.usage,
  };
}

function isContentBlock(value: unknown): value is ContentBlock {
  return (
    Boolean(value) &&
    typeof value === "object" &&
    (value as { object?: unknown }).object === "content"
  );
}

function userMessage(content: OutboundContentBlock[]): StreamMessage {
  const id = `local_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
  return {
    id,
    type: "message",
    role: "user",
    status: "completed",
    metadata: null,
    content: content.map((block, index) => localContentBlock(block, index, id)),
  };
}

function localContentBlock(
  block: OutboundContentBlock,
  index: number,
  messageId: string,
): ContentBlock {
  const common = {
    object: "content" as const,
    delta: false,
    index,
    status: null,
    msg_id: messageId,
  };
  if (block.type === "image") return { ...common, ...block };
  if (block.type === "file") return { ...common, ...block };
  return { ...common, ...block };
}

function readableError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function isAbort(error: unknown) {
  return error instanceof DOMException && error.name === "AbortError";
}

function revokePreviews(images: PendingImage[]) {
  images.forEach((image) => {
    if (image.previewUrl) URL.revokeObjectURL(image.previewUrl);
  });
}

function isApprovalLevel(value: string | undefined): value is ApprovalLevel {
  return (
    value === "STRICT" ||
    value === "SMART" ||
    value === "AUTO" ||
    value === "OFF"
  );
}

function delay(milliseconds: number) {
  return new Promise<void>((resolve) => {
    window.setTimeout(resolve, milliseconds);
  });
}

function isTerminalStatus(status: ConversationStreamState["responseStatus"]) {
  return (
    status === "completed" || status === "failed" || status === "cancelled"
  );
}

async function uploadPendingFiles(
  files: File[],
): Promise<UploadedAttachment[]> {
  if (files.length === 0) return [];

  let uploadLimit: Awaited<ReturnType<typeof settingsApi.uploadLimit>>;
  try {
    uploadLimit = await settingsApi.uploadLimit();
  } catch (error) {
    throw new Error(
      t("attachment.limitCheckFailed", { message: readableError(error) }),
    );
  }

  const oversized = findOversizedFile(files, uploadLimit.upload_max_size_mb);
  if (oversized) {
    throw new Error(
      t("attachment.tooLarge", {
        name: oversized.name,
        limit: uploadLimit.upload_max_size_mb ?? 0,
      }),
    );
  }

  const uploaded: UploadedAttachment[] = [];
  for (const file of files) {
    try {
      const response = await chatApi.upload(file);
      uploaded.push({
        url: response.url,
        filename: response.file_name || file.name,
        mimeType: file.type,
      });
    } catch (error) {
      throw new Error(
        t("attachment.uploadFailed", {
          name: file.name,
          message: readableError(error),
        }),
      );
    }
  }
  return uploaded;
}
