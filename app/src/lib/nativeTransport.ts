import { hasDesktopHostBridge, invokeDesktop, listenDesktopEvent } from "./desktop";

let discovery: Promise<boolean> | null = null;
let enabled = false;

export function nativeRuntimeKnown(): boolean { return enabled; }

export function detectNativeRuntime(): Promise<boolean> {
  if (!hasDesktopHostBridge()) return Promise.resolve(false);
  discovery ??= invokeDesktop<boolean>("native_runtime_enabled")
    .then((value) => { enabled = value === true; return enabled; })
    .catch(() => false);
  return discovery;
}

function errorResponse(error: unknown): Response {
  const value = error as { status?: number; message?: string } | null;
  const status = value?.status && value.status >= 400 && value.status <= 599 ? value.status : 500;
  return Response.json({ detail: value?.message ?? String(error) }, { status });
}

/** Adapt IPC to the existing API/SSE contract so the React views stay shared. */
export async function nativeFetch(path: string, init: RequestInit): Promise<Response> {
  if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
  if (path === "/api/console/chat" && init.method === "POST") {
    return nativeChat(JSON.parse(String(init.body)), init.signal);
  }
  try {
    if (init.body instanceof FormData) {
      const file = init.body.get("file");
      if (!(file instanceof File)) return errorResponse({ status: 400, message: "File is required" });
      if (file.size > 20_000_000) return errorResponse({ status: 413, message: "File exceeds 20 MB" });
      if (path === "/api/skills/upload") {
        const bytes = new Uint8Array(await file.arrayBuffer());
        let binary = "";
        for (let offset = 0; offset < bytes.length; offset += 8192) binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
        return Response.json(await invokeDesktop("native_request", {
          method: "POST", path, body: { filename: file.name, base64: btoa(binary) },
        }));
      }
      if (path === "/api/console/upload") {
        const image = /^image\/(png|jpeg|webp|gif)$/.test(file.type);
        const bytes = new Uint8Array(await file.arrayBuffer());
        let binary = "";
        for (let offset = 0; offset < bytes.length; offset += 8192) {
          binary += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
        }
        if (!image) return Response.json(await invokeDesktop("native_request", {
          method: "POST", path, body: { filename: file.name, base64: btoa(binary) },
        }));
        return Response.json({ url: `data:${image ? file.type : "text/plain"};base64,${btoa(binary)}`, file_name: file.name, size: file.size });
      }
      if (path !== "/api/workspace/transcribe") return errorResponse({ status: 501, message: "This upload has not been migrated" });
      return Response.json(await invokeDesktop("native_transcribe", {
        filename: file.name, mime: file.type || "audio/wav", bytes: Array.from(new Uint8Array(await file.arrayBuffer())),
      }));
    }
    const value = await invokeDesktop<unknown>("native_request", {
      method: init.method ?? "GET", path,
      body: init.body ? JSON.parse(String(init.body)) : null,
    });
    if (init.signal?.aborted) throw new DOMException("Aborted", "AbortError");
    if (path === "/api/workspace/download" && value && typeof value === "object" && "native_binary" in value) {
      const binary = value as { native_binary: string; mime: string };
      const bytes = Uint8Array.from(atob(binary.native_binary), (char) => char.charCodeAt(0));
      return new Response(bytes, { headers: { "Content-Type": binary.mime } });
    }
    return Response.json(value);
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") throw error;
    return errorResponse(error);
  }
}

async function nativeChat(body: unknown, signal?: AbortSignal | null): Promise<Response> {
  const requestId = crypto.randomUUID();
  let controller: ReadableStreamDefaultController<Uint8Array>;
  let closed = false;
  let unlisten: (() => void) | null = null;
  const encoder = new TextEncoder();
  const cleanup = () => { unlisten?.(); unlisten = null; signal?.removeEventListener("abort", abort); };
  const cancel = () => { void invokeDesktop("native_chat_cancel", { requestId }).catch(() => undefined); };
  const abort = () => {
    if (closed) return;
    closed = true; cleanup(); cancel();
    controller.error(new DOMException("Aborted", "AbortError"));
  };
  const stream = new ReadableStream<Uint8Array>({
    start(value) { controller = value; },
    cancel() { if (!closed) { closed = true; cleanup(); cancel(); } },
  });
  unlisten = await listenDesktopEvent<Record<string, unknown>>(`native-chat-${requestId}`, (frame) => {
    if (closed) return;
    controller.enqueue(encoder.encode(`data: ${JSON.stringify(frame)}\n\n`));
    if (frame.object === "response" && ["completed", "failed", "cancelled"].includes(String(frame.status))) {
      closed = true; cleanup(); controller.close();
    }
  });
  if (!unlisten) return errorResponse({ status: 500, message: "Native stream listener could not be registered" });
  signal?.addEventListener("abort", abort, { once: true });
  if (signal?.aborted) { abort(); throw new DOMException("Aborted", "AbortError"); }
  try {
    await invokeDesktop("native_chat_start", { requestId, body });
    // Abort can race with command admission; cancel again after its acknowledgement.
    if (signal?.aborted) { cancel(); throw new DOMException("Aborted", "AbortError"); }
    return new Response(stream, { headers: { "Content-Type": "text/event-stream" } });
  } catch (error) {
    if (!closed) { closed = true; cleanup(); controller!.close(); }
    if (error instanceof DOMException && error.name === "AbortError") throw error;
    return errorResponse(error);
  }
}
