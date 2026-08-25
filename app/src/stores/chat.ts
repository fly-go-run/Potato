import { create, type StoreApi } from "zustand";
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
import { waitForBackendOrigin } from "../lib/backendOrigin";
import { t } from "../lib/i18n";
import { sortChats } from "../lib/chats";
import { resetMessageTimings, trackMessageTimings } from "../lib/messageTiming";
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
export type SandboxMode =
  | "read-only"
  | "workspace-write"
  | "danger-full-access";

const SANDBOX_MODE_KEY = "potato.sandbox_mode";
const APPROVAL_LEVEL_KEY = "potato.approval_level";

function loadSandboxMode(): SandboxMode {
  try {
    const raw = globalThis.localStorage?.getItem(SANDBOX_MODE_KEY);
    if (
      raw === "read-only" ||
      raw === "workspace-write" ||
      raw === "danger-full-access"
    ) {
      return raw;
    }
  } catch {
    // localStorage may be blocked in some embeds
  }
  return "workspace-write";
}

function persistSandboxMode(mode: SandboxMode) {
  try {
    globalThis.localStorage?.setItem(SANDBOX_MODE_KEY, mode);
  } catch {
    // ignore quota / privacy mode
  }
}

function loadStoredApprovalLevel(): ApprovalLevel {
  try {
    const raw = globalThis.localStorage?.getItem(APPROVAL_LEVEL_KEY);
    if (
      raw === "STRICT" ||
      raw === "SMART" ||
      raw === "AUTO" ||
      raw === "OFF"
    ) {
      return raw;
    }
  } catch {
    // localStorage may be blocked in some embeds
  }
  return "AUTO";
}

function persistApprovalLevel(level: ApprovalLevel) {
  try {
    globalThis.localStorage?.setItem(APPROVAL_LEVEL_KEY, level);
  } catch {
    // ignore quota / privacy mode
  }
}

function persistRunningPermissions(patch: {
  approval_level?: ApprovalLevel;
  sandbox_mode?: SandboxMode;
}) {
  void workspaceApi
    .runningConfig()
    .then((current) =>
      workspaceApi.putRunningConfig({
        ...current,
        ...patch,
      }),
    )
    .catch(() => {
      // session override still lives in memory / localStorage
    });
}

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
  sandboxMode: SandboxMode;
  project: ProjectBinding | null;
  pendingImages: PendingImage[];
  pendingApprovals: PendingApproval[];
  composerDraft: string | null;
  requestController: AbortController | null;
  followupController: AbortController | null;
  queuedMessageIds: string[];

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
  setSandboxMode: (mode: SandboxMode) => void;
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
const PENDING_SESSION_KEY = "potato_pending_chat_session";

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
  approvalLevel: loadStoredApprovalLevel(),
  sandboxMode: loadSandboxMode(),
  project: initialProject(),
  pendingImages: [],
  pendingApprovals: [],
  composerDraft: null,
  requestController: null,
  followupController: null,
  queuedMessageIds: [],

  initialize: async () => {
    try {
      await waitForBackendOrigin();
    } catch {
      set({ error: t("desktop.backend.error") });
      return;
    }
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
      if (isApprovalLevel(level)) {
        persistApprovalLevel(level);
        set({ approvalLevel: level });
      }
      const mode = config.sandbox_mode;
      if (
        mode === "read-only" ||
        mode === "workspace-write" ||
        mode === "danger-full-access"
      ) {
        persistSandboxMode(mode);
        set({ sandboxMode: mode });
      }
    } catch {
      // AUTO remains the contract-compatible fallback.
    }
  },

  newChat: () => {
    get().requestController?.abort();
    get().followupController?.abort();
    sessionStorage.removeItem(PENDING_SESSION_KEY);
    revokePreviews(get().pendingImages);
    resetMessageTimings();
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
      followupController: null,
      queuedMessageIds: [],
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
    get().followupController?.abort();
    resetMessageTimings();
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
      followupController: null,
      queuedMessageIds: [],
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
    if ((!text && get().pendingImages.length === 0) || get().isSubmitting) {
      return false;
    }
    if (get().isStreaming) {
      return enqueueFollowup(text, set, get);
    }
    if (isUnfinishedResponse(get().stream.responseStatus)) {
      const message = t("stream.turnStillRunning");
      set((state) => ({
        error: message,
        stream: { ...state.stream, error: message },
      }));
      return false;
    }

    const submittedImages = get().pendingImages;
    set({ isSubmitting: true, error: null });
    try {
      await waitForBackendOrigin();
      if (!get().activeModel?.active_llm) await get().loadActiveModel();
    } catch {
      set({ isSubmitting: false, error: t("desktop.backend.error") });
      return false;
    }
    if (!get().isSubmitting) return false;
    const model = get().activeModel?.active_llm;
    if (!model) {
      set({ isSubmitting: false, error: t("chat.modelRequired") });
      return false;
    }
    let uploadedAttachments: UploadedAttachment[];
    try {
      uploadedAttachments = await uploadPendingFiles(
        submittedImages.map((attachment) => attachment.file),
      );
    } catch (error) {
      set({ isSubmitting: false, error: readableError(error) });
      return false;
    }
    if (!get().isSubmitting) return false;

    const outboundContent = buildOutboundContent(text, uploadedAttachments);
    const localMessage = userMessage(outboundContent);
    const controller = new AbortController();
    const previousStream = get().stream;
    const baseStream: ConversationStreamState = {
      ...previousStream,
      responseId: null,
      responseStatus: "created",
      messages: [...previousStream.messages, localMessage],
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

    let requestAccepted = false;
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
            sandbox_mode: get().sandboxMode,
            ...(get().project
              ? { "potato.coding_project_dir": get().project?.path }
              : {}),
          },
        },
        controller.signal,
      );
      requestAccepted = true;
      revokePreviews(submittedImages);
      const submittedImageIds = new Set(
        submittedImages.map((attachment) => attachment.id),
      );
      set((state) => ({
        pendingImages: state.pendingImages.filter(
          (attachment) => !submittedImageIds.has(attachment.id),
        ),
      }));

      const navigationDone = get()
        .refreshChats()
        .then((chats) => {
          const created = chats.find((chat) => chat.session_id === sessionId);
          if (created && get().sessionId === sessionId) {
            set({ activeChatId: created.id });
            navigate(`/chat/${created.id}`, { replace: true });
          }
        })
        .catch(() => {});
      await consumeResponse(response, controller, set, get);
      await navigationDone;
    } catch (error) {
      if (!isAbort(error) && !controller.signal.aborted) {
        const message = readableError(error);
        const knownChat = requestAccepted
          ? get().chats.find((chat) => chat.session_id === sessionId) ??
            (await get().refreshChats()).find(
              (chat) => chat.session_id === sessionId,
            )
          : (await get().refreshChats()).find(
              (chat) => chat.session_id === sessionId,
            );
        if (controller.signal.aborted) return false;
        if (knownChat?.status === "running") {
          if (requestAccepted) {
            await get().reconnect(knownChat);
          } else {
            set({ error: null, stream: previousStream });
            void get().reconnect(knownChat);
          }
        } else {
          set((state) => ({
            stream: {
              ...(requestAccepted ? state.stream : previousStream),
              responseStatus:
                error instanceof ApiError
                  ? "failed"
                  : requestAccepted
                  ? state.stream.responseStatus
                  : previousStream.responseStatus,
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
          (!requestAccepted || isTerminalStatus(get().stream.responseStatus))
        ) {
          sessionStorage.removeItem(PENDING_SESSION_KEY);
        }
        set({
          isStreaming: false,
          isSubmitting: false,
          requestController: null,
          pendingApprovals: [],
          queuedMessageIds: [],
        });
      }
      await get().refreshChats();
    }
    return requestAccepted;
  },

  stop: async () => {
    const {
      activeChatId,
      requestController,
      followupController,
      pendingImages,
      sessionId,
    } = get();
    // Drop local follow-ups before aborting the SSE. sendMessage's
    // finally clears queuedMessageIds without removing messages; if
    // abort wins that race the bubbles would stick.
    followupController?.abort();
    requestController?.abort();
    revokePreviews(pendingImages);
    if (
      !activeChatId &&
      sessionStorage.getItem(PENDING_SESSION_KEY) === sessionId
    ) {
      sessionStorage.removeItem(PENDING_SESSION_KEY);
    }
    set((state) => ({
      isStreaming: false,
      isSubmitting: false,
      requestController: null,
      followupController: null,
      pendingImages: [],
      pendingApprovals: [],
      ...dropQueuedFollowups(state),
    }));
    if (!activeChatId) return;
    try {
      await chatApi.stop(activeChatId);
      await get().refreshChats();
    } catch (error) {
      set({ error: readableError(error) });
    }
  },

  reconnect: async (chat) => {
    get().requestController?.abort();
    get().followupController?.abort();
    const controller = new AbortController();
    set({
      activeChatId: chat.id,
      sessionId: chat.session_id,
      userId: chat.user_id,
      channel: chat.channel,
      isStreaming: true,
      isSubmitting: false,
      error: null,
      pendingApprovals: [],
      requestController: controller,
      followupController: null,
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
          set((state) => {
            const fromHistory = historyMessages(history);
            const historyIds = new Set(fromHistory.map((item) => item.id));
            const extras = state.stream.messages.filter(
              (message) =>
                state.queuedMessageIds.includes(message.id) &&
                !historyIds.has(message.id),
            );
            return {
              stream: {
                ...state.stream,
                messages: [...fromHistory, ...extras],
                turnUsage: historyTurnUsage(history, chat.session_id),
                responseStatus:
                  history.status === "running" ? "in_progress" : "completed",
              },
            };
          });
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

  setApprovalLevel: (approvalLevel) => {
    persistApprovalLevel(approvalLevel);
    set({ approvalLevel });
    persistRunningPermissions({ approval_level: approvalLevel });
  },
  setSandboxMode: (sandboxMode) => {
    persistSandboxMode(sandboxMode);
    set({ sandboxMode });
    persistRunningPermissions({ sandbox_mode: sandboxMode });
  },
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
        const pendingApprovals = filterApprovalsForSession(
          response.pending_approvals,
          sessionId,
        );
        const current = get().pendingApprovals;
        const unchanged =
          current.length === pendingApprovals.length &&
          current.every(
            (approval, index) =>
              approval.request_id === pendingApprovals[index]?.request_id,
          );
        if (!unchanged) set({ pendingApprovals });
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

async function enqueueFollowup(
  text: string,
  set: StoreApi<ChatStore>["setState"],
  get: StoreApi<ChatStore>["getState"],
): Promise<boolean> {
  const submittedImages = get().pendingImages;
  set({ isSubmitting: true, error: null });
  let uploadedAttachments: UploadedAttachment[];
  try {
    uploadedAttachments = await uploadPendingFiles(
      submittedImages.map((attachment) => attachment.file),
    );
  } catch (error) {
    set({ isSubmitting: false, error: readableError(error) });
    return false;
  }
  if (!get().isSubmitting) return false;
  const outboundContent = buildOutboundContent(text, uploadedAttachments);
  const localMessage = userMessage(outboundContent);
  let followupController = get().followupController;
  if (!followupController || followupController.signal.aborted) {
    followupController = new AbortController();
  }
  set((state) => ({
    stream: {
      ...state.stream,
      messages: [...state.stream.messages, localMessage],
      error: null,
    },
    queuedMessageIds: [...state.queuedMessageIds, localMessage.id],
    followupController,
    isSubmitting: false,
    error: null,
  }));
  const removeLocal = (error: string | null) =>
    set((state) => ({
      error,
      queuedMessageIds: state.queuedMessageIds.filter(
        (id) => id !== localMessage.id,
      ),
      stream: {
        ...state.stream,
        messages: state.stream.messages.filter(
          (message) => message.id !== localMessage.id,
        ),
      },
    }));
  try {
    const response = await chatApi.stream(
      {
        input: [{ role: "user", content: outboundContent }],
        session_id: get().sessionId,
        user_id: get().userId,
        channel: get().channel,
        stream: true,
        request_context: {
          approval_level: get().approvalLevel,
          sandbox_mode: get().sandboxMode,
          ...(get().project
            ? { "potato.coding_project_dir": get().project?.path }
            : {}),
        },
      },
      followupController.signal,
    );
    if (response.status === 202 || response.status === 200) {
      revokePreviews(submittedImages);
      const submittedImageIds = new Set(
        submittedImages.map((attachment) => attachment.id),
      );
      set((state) => ({
        pendingImages: state.pendingImages.filter(
          (attachment) => !submittedImageIds.has(attachment.id),
        ),
      }));
      if (response.status === 200) {
        const controller = new AbortController();
        get().requestController?.abort();
        set({ requestController: controller, isStreaming: true });
        try {
          await consumeResponse(response, controller, set, get);
        } catch (error) {
          if (!isAbort(error) && !controller.signal.aborted) {
            const knownChat = get().chats.find(
              (item) => item.session_id === get().sessionId,
            );
            if (knownChat?.status === "running") {
              await get().reconnect(knownChat);
            } else {
              set({ error: readableError(error) });
            }
          }
        } finally {
          if (get().requestController === controller) {
            set({
              isStreaming: false,
              requestController: null,
              queuedMessageIds: [],
            });
          }
          await get().refreshChats();
        }
      }
      return true;
    }
    removeLocal(t("stream.turnStillRunning"));
    return false;
  } catch (error) {
    if (isAbort(error)) {
      removeLocal(null);
    } else {
      removeLocal(readableError(error));
    }
    return false;
  }
}

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
  let pending = get().stream;
  let hasPending = false;
  let lastFlushAt = Date.now();
  let flushTimer: ReturnType<typeof setTimeout> | null = null;

  const clearFlushTimer = () => {
    if (flushTimer !== null) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
  };
  const flush = () => {
    clearFlushTimer();
    if (!hasPending || controller.signal.aborted) return;
    set((state) => {
      const stream = mergeLiveLocalMessages(pending, state.stream);
      pending = stream;
      return { stream, error: stream.error };
    });
    hasPending = false;
    lastFlushAt = Date.now();
  };
  const scheduleFlush = () => {
    if (flushTimer !== null || controller.signal.aborted) return;
    const delay = Math.max(0, 40 - (Date.now() - lastFlushAt));
    flushTimer = setTimeout(() => {
      flushTimer = null;
      flush();
    }, delay);
  };

  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      if (controller.signal.aborted) break;
      const parsed = parseSseBytes(value, parser);
      parser = parsed.state;
      for (const frame of parsed.frames) {
        const previous = pending;
        pending = reduceStreamFrame(pending, frame);
        trackMessageTimings(previous, pending);
        hasPending = hasPending || pending !== previous;
        const flushImmediately =
          pending.responseStatus !== previous.responseStatus ||
          pending.error !== previous.error ||
          pending.rateLimited !== previous.rateLimited;
        if (flushImmediately) flush();
        else if (hasPending) scheduleFlush();
      }
    }
  } finally {
    flush();
    clearFlushTimer();
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
    const usageMetadata = nested as {
      potato_turn_usage?: unknown;
      qwenpaw_turn_usage?: unknown;
    };
    const snapshot =
      usageMetadata.potato_turn_usage ?? usageMetadata.qwenpaw_turn_usage;
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

function mergeLiveLocalMessages(
  pending: ConversationStreamState,
  live: ConversationStreamState,
): ConversationStreamState {
  const pendingIds = new Set(pending.messages.map((message) => message.id));
  const extras = live.messages.filter(
    (message) =>
      !pendingIds.has(message.id) && message.id.startsWith("local_"),
  );
  if (extras.length === 0) return pending;
  return {
    ...pending,
    messages: [...pending.messages, ...extras],
  };
}

function dropQueuedFollowups(state: {
  stream: ConversationStreamState;
  queuedMessageIds: string[];
}): {
  stream: ConversationStreamState;
  queuedMessageIds: string[];
} {
  const drop = new Set(state.queuedMessageIds);
  return {
    queuedMessageIds: [],
    stream: {
      ...state.stream,
      responseStatus: "cancelled",
      messages: state.stream.messages.filter((message) => !drop.has(message.id)),
    },
  };
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
