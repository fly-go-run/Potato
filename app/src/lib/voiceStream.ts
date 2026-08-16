import { getAuthToken } from "./api";
import { getBackendOrigin } from "./backendOrigin";

export type VoiceStreamError = {
  code: string;
  message: string;
};

export type VoiceStreamHandlers = {
  onPartial: (text: string) => void;
  onFinal?: (text: string) => void;
  onError: (error: VoiceStreamError) => void;
};

export type VoiceStreamSession = {
  sendPcm: (pcm: ArrayBuffer) => void;
  stop: () => Promise<string>;
  cancel: () => void;
};

export function transcribeStreamUrl(): string {
  const token = getAuthToken();
  const query = token ? `?token=${encodeURIComponent(token)}` : "";
  const httpOrigin = getBackendOrigin() || window.location.origin;
  let host = window.location.host;
  let secure = window.location.protocol === "https:";
  try {
    const parsed = new URL(httpOrigin);
    host = parsed.host;
    secure = parsed.protocol === "https:";
  } catch {
    // Fall back to the page origin when the sidecar origin is unset.
  }
  const protocol = secure ? "wss:" : "ws:";
  return `${protocol}//${host}/api/workspace/transcribe-stream${query}`;
}

/**
 * Open a live transcription socket. Resolves after the backend has
 * connected to Doubao and sent ``ready`` — so a dead key fails before
 * the user starts talking.
 */
export function openVoiceStream(
  handlers: VoiceStreamHandlers,
): Promise<VoiceStreamSession> {
  return new Promise((resolve, reject) => {
    let socket: WebSocket;
    try {
      socket = new WebSocket(transcribeStreamUrl());
    } catch (reason) {
      reject(reason);
      return;
    }
    socket.binaryType = "arraybuffer";

    let settled = false;
    let lastText = "";
    let stopWaiter: ((text: string) => void) | null = null;
    const fail = (error: VoiceStreamError) => {
      if (stopWaiter) {
        const wait = stopWaiter;
        stopWaiter = null;
        wait(lastText);
      }
      if (!settled) {
        settled = true;
        reject(Object.assign(new Error(error.message), { code: error.code }));
        return;
      }
      handlers.onError(error);
    };

    const timer = window.setTimeout(() => {
      fail({
        code: "TRANSCRIPTION_FAILED",
        message: "Streaming speech recognition timed out",
      });
      socket.close();
    }, 12_000);

    socket.onmessage = (event) => {
      if (typeof event.data !== "string") return;
      let payload: {
        type?: string;
        text?: string;
        code?: string;
        message?: string;
      };
      try {
        payload = JSON.parse(event.data) as typeof payload;
      } catch {
        return;
      }
      if (payload.type === "ready") {
        if (settled) return;
        settled = true;
        window.clearTimeout(timer);
        resolve({
          sendPcm: (pcm) => {
            if (socket.readyState === WebSocket.OPEN) socket.send(pcm);
          },
          stop: () =>
            new Promise((done) => {
              stopWaiter = done;
              if (socket.readyState === WebSocket.OPEN) {
                socket.send(JSON.stringify({ type: "stop" }));
              } else {
                done(lastText);
                return;
              }
              window.setTimeout(() => {
                if (!stopWaiter) return;
                stopWaiter = null;
                done(lastText);
                socket.close();
              }, 2500);
            }),
          cancel: () => {
            stopWaiter = null;
            if (
              socket.readyState === WebSocket.OPEN ||
              socket.readyState === WebSocket.CONNECTING
            ) {
              socket.close();
            }
          },
        });
        return;
      }
      if (payload.type === "error") {
        window.clearTimeout(timer);
        fail({
          code: payload.code || "TRANSCRIPTION_FAILED",
          message: payload.message || "Streaming speech recognition failed",
        });
        socket.close();
        return;
      }
      const text = (payload.text ?? "").trim();
      if (text) {
        lastText = text;
        handlers.onPartial(text);
      }
      if (payload.type === "final") {
        if (text) handlers.onFinal?.(text);
        if (stopWaiter) {
          const wait = stopWaiter;
          stopWaiter = null;
          wait(lastText);
        }
        socket.close();
      }
    };

    socket.onerror = () => {
      window.clearTimeout(timer);
      fail({
        code: "TRANSCRIPTION_FAILED",
        message: "Streaming speech recognition failed",
      });
    };
    socket.onclose = () => {
      window.clearTimeout(timer);
      if (stopWaiter) {
        const wait = stopWaiter;
        stopWaiter = null;
        wait(lastText);
      }
      if (!settled) {
        fail({
          code: "TRANSCRIPTION_FAILED",
          message: "Streaming speech recognition closed",
        });
      }
    };
  });
}
