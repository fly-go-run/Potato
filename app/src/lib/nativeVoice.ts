import { invokeDesktop, listenDesktopEvent } from "./desktop";
import type { VoiceStreamHandlers, VoiceStreamSession } from "./voiceStream";

export async function openNativeVoice(handlers: VoiceStreamHandlers): Promise<VoiceStreamSession> {
  const requestId = crypto.randomUUID();
  let closed = false;
  let stopping = false;
  let lastText = "";
  let waiter: ((value: string) => void) | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let unlisten: (() => void) | null = null;
  let sends = Promise.resolve();
  let queued = 0;
  const close = () => { closed = true; clearTimeout(timer); unlisten?.(); unlisten = null; };
  const fail = (reason: unknown) => {
    if (closed) return;
    const error = reason as { message?: string };
    close(); waiter?.(lastText); waiter = null;
    void invokeDesktop("native_voice_end", { requestId, cancel: true }).catch(() => undefined);
    handlers.onError({ code: "TRANSCRIPTION_FAILED", message: error?.message || "语音识别失败" });
  };
  unlisten = await listenDesktopEvent<{ type: string; text?: string; message?: string }>(`native-voice-${requestId}`, (frame) => {
    if (closed) return;
    if (frame.type === "error") { fail(frame); return; }
    if (typeof frame.text === "string") lastText = frame.text;
    if (frame.type === "partial") handlers.onPartial(lastText);
    if (frame.type === "final") { close(); handlers.onFinal?.(lastText); waiter?.(lastText); waiter = null; }
  });
  if (!unlisten) throw new Error("无法注册语音识别监听器");
  try { await invokeDesktop("native_voice_start", { requestId }); }
  catch (reason) { close(); throw new Error((reason as { message?: string })?.message || "无法连接豆包语音识别"); }
  // An early server error can arrive before IPC start resolves.
  if (closed) throw new Error("豆包语音连接在录音开始前关闭");
  return {
    sendPcm(pcm) {
      if (closed || stopping) return;
      if (++queued > 32) { queued--; fail(new Error("语音服务处理速度不足，请重试")); return; }
      const bytes = Array.from(new Uint8Array(pcm));
      sends = sends.then(async () => {
        if (!closed) await invokeDesktop("native_voice_audio", { requestId, bytes });
      }).catch(fail).finally(() => { queued--; });
    },
    stop() {
      if (closed) return Promise.resolve(lastText);
      if (stopping) return Promise.reject(new Error("Voice recording is already stopping"));
      stopping = true;
      return new Promise((resolve) => {
        waiter = resolve;
        timer = setTimeout(() => fail(new Error("语音识别等待结果超时")), 10_000);
        void sends.then(() => {
          if (!closed) return invokeDesktop("native_voice_end", { requestId, cancel: false });
        }).catch(fail);
      });
    },
    cancel() {
      if (closed) return;
      close(); waiter?.(lastText); waiter = null;
      void invokeDesktop("native_voice_end", { requestId, cancel: true }).catch(() => undefined);
    },
  };
}
