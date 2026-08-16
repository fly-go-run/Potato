/**
 * 麦克风采集。直接产出 16k 单声道 WAV。
 *
 * 早先走 MediaRecorder,浏览器只会给 webm(Chromium)或 mp4(WKWebView),
 * 而 ASR 端点只收 wav/mp3/ogg,于是后端必须用 ffmpeg 转码——打包后的桌面
 * App 既没有随包的 ffmpeg,也拿不到登录 shell 的 PATH,语音就整个不可用。
 * 这里改成用 Web Audio 采 PCM 自己封 WAV:输出就是 ffmpeg 原本要转成的
 * 那个格式,不再依赖任何外部二进制,Windows / macOS 表现一致。
 */

import { ApiError } from "./api";

export type VoiceInputErrorCode =
  | "unsupported"
  | "permission"
  | "empty"
  | "too_short"
  | "start_failed";

const FATAL_SPEECH_CODES = new Set([
  "TRANSCRIPTION_DISABLED",
  "SPEECH_API_KEY_MISSING",
  "AUDIO_CONVERSION_UNAVAILABLE",
]);

function speechErrorCode(reason: unknown): string {
  if (reason instanceof ApiError) return reason.code;
  if (reason && typeof reason === "object" && "code" in reason) {
    const code = (reason as { code: unknown }).code;
    return typeof code === "string" ? code : "";
  }
  return "";
}

/** Config / environment errors should abort live listening immediately. */
export function isFatalSpeechError(reason: unknown): boolean {
  return FATAL_SPEECH_CODES.has(speechErrorCode(reason));
}

/** Silence / "no speech" on a short live slice is not a reason to abort. */
export function isBenignSpeechMiss(reason: unknown): boolean {
  if (!(reason instanceof ApiError)) return false;
  if (isFatalSpeechError(reason)) return false;
  const message = `${reason.code} ${reason.message}`.toLowerCase();
  return /no valid speech|no speech|empty|too short/.test(message);
}

/** Replace the live draft after the pre-recording composer text. */
export function joinVoiceDraft(
  base: string,
  transcript: string,
): string | null {
  const piece = transcript.trim();
  if (!piece) return null;
  if (!base.trim()) return piece;
  return `${base.replace(/\s+$/, "")} ${piece}`;
}

export class VoiceInputError extends Error {
  constructor(
    readonly code: VoiceInputErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "VoiceInputError";
  }
}

const MIN_DURATION_MS = 400;
const MAX_DURATION_MS = 120_000;
/** ASR 只需要 16k 单声道,和原先 ffmpeg 的转码参数一致。 */
export const TARGET_SAMPLE_RATE = 16_000;
/** ScriptProcessor 缓冲区:2048 帧 = 128ms @16k / 43ms @48k。 */
const BUFFER_SIZE = 2048;
/** 收尾时等最后一块缓冲的上限,略大于一个缓冲周期。 */
const FLUSH_TIMEOUT_MS = 300;

/**
 * 重采样到 *toRate*。
 *
 * 降采样走窗口平均而不是点采样:48k→16k 直接抽点会把 8kHz 以上的成分
 * 折叠进语音频段,而 WAV 头已经写成 16k,后端无从补救。窗口平均是个粗
 * 糙的低通,对 ASR 足够,也不必为此引入 DSP 库。升采样(少见)用线性插值。
 */
export function resampleTo(
  input: Float32Array,
  fromRate: number,
  toRate: number,
): Float32Array {
  if (fromRate === toRate || input.length === 0) return input;
  const ratio = fromRate / toRate;
  const length = Math.floor(input.length / ratio);
  const output = new Float32Array(length);
  if (ratio > 1) {
    for (let index = 0; index < length; index += 1) {
      const start = Math.floor(index * ratio);
      const end = Math.min(Math.floor((index + 1) * ratio), input.length);
      let sum = 0;
      for (let cursor = start; cursor < end; cursor += 1) {
        sum += input[cursor]!;
      }
      output[index] = end > start ? sum / (end - start) : input[start]!;
    }
    return output;
  }
  for (let index = 0; index < length; index += 1) {
    const position = index * ratio;
    const left = Math.floor(position);
    const right = Math.min(left + 1, input.length - 1);
    const weight = position - left;
    output[index] = input[left]! * (1 - weight) + input[right]! * weight;
  }
  return output;
}

/** 把 [-1,1] 的浮点采样封成 16-bit PCM 的 WAV。 */
export function encodeWav(
  samples: Float32Array,
  sampleRate: number,
): ArrayBuffer {
  const buffer = new ArrayBuffer(44 + samples.length * 2);
  const view = new DataView(buffer);
  const writeAscii = (offset: number, text: string) => {
    for (let index = 0; index < text.length; index += 1) {
      view.setUint8(offset + index, text.charCodeAt(index));
    }
  };

  writeAscii(0, "RIFF");
  view.setUint32(4, 36 + samples.length * 2, true);
  writeAscii(8, "WAVE");
  writeAscii(12, "fmt ");
  view.setUint32(16, 16, true); // PCM 头长度
  view.setUint16(20, 1, true); // PCM
  view.setUint16(22, 1, true); // 单声道
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true); // 字节率
  view.setUint16(32, 2, true); // 块对齐
  view.setUint16(34, 16, true); // 位深
  writeAscii(36, "data");
  view.setUint32(40, samples.length * 2, true);

  let offset = 44;
  for (const sample of samples) {
    const clamped = Math.max(-1, Math.min(1, sample));
    view.setInt16(offset, clamped * 0x7fff, true);
    offset += 2;
  }
  return buffer;
}

/** Convert [-1,1] floats to little-endian PCM16. */
export function floatToPcm16(samples: Float32Array): ArrayBuffer {
  const buffer = new ArrayBuffer(samples.length * 2);
  const view = new DataView(buffer);
  for (let index = 0; index < samples.length; index += 1) {
    const clamped = Math.max(-1, Math.min(1, samples[index]!));
    view.setInt16(index * 2, clamped * 0x7fff, true);
  }
  return buffer;
}

function mergeChunks(chunks: Float32Array[]): Float32Array {
  let total = 0;
  for (const chunk of chunks) total += chunk.length;
  const merged = new Float32Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.length;
  }
  return merged;
}

type AudioContextCtor = typeof AudioContext;

function audioContextCtor(): AudioContextCtor | undefined {
  if (typeof window === "undefined") return undefined;
  const scoped = window as typeof window & {
    webkitAudioContext?: AudioContextCtor;
  };
  return window.AudioContext ?? scoped.webkitAudioContext;
}

export class VoiceRecorder {
  private context: AudioContext | null = null;
  private source: MediaStreamAudioSourceNode | null = null;
  private processor: ScriptProcessorNode | null = null;
  private silence: GainNode | null = null;
  private stream: MediaStream | null = null;
  private chunks: Float32Array[] = [];
  private startedAt = 0;
  private maxTimer: ReturnType<typeof setTimeout> | null = null;
  private active = false;
  /** stop() 在等下一次 onaudioprocess 交出尾块。 */
  private pendingFlush: (() => void) | null = null;
  /** 去重:手动停止与两分钟自动停止可能同时到。 */
  private stopPromise: Promise<File> | null = null;
  /** 代次令牌:start() 的每个 await 之后都要确认这次启动还算数。 */
  private generation = 0;
  private onPcm: ((pcm: ArrayBuffer) => void) | null = null;
  private pendingNative: Float32Array[] = [];
  private pendingNativeFrames = 0;

  get recording(): boolean {
    return this.active;
  }

  /**
   * Encode audio captured so far without stopping the mic.
   * Used for live partial transcripts while the user is still talking.
   */
  snapshot(): File | null {
    if (!this.active || this.chunks.length === 0) return null;
    const sampleRate = this.context?.sampleRate ?? TARGET_SAMPLE_RATE;
    const captured = mergeChunks(this.chunks);
    if (!captured.length) return null;
    const samples = resampleTo(captured, sampleRate, TARGET_SAMPLE_RATE);
    const wav = encodeWav(samples, TARGET_SAMPLE_RATE);
    return new File([wav], `voice-partial-${Date.now()}.wav`, {
      type: "audio/wav",
    });
  }

  async start(
    onAutoStop?: (file: File) => void,
    onPcm?: (pcm: ArrayBuffer) => void,
  ): Promise<void> {
    const Ctor = audioContextCtor();
    if (
      typeof navigator === "undefined" ||
      !navigator.mediaDevices?.getUserMedia ||
      !Ctor
    ) {
      throw new VoiceInputError(
        "unsupported",
        "Web Audio capture is not available in this environment",
      );
    }

    const generation = ++this.generation;
    this.chunks = [];
    this.pendingNative = [];
    this.pendingNativeFrames = 0;
    this.onPcm = onPcm ?? null;
    try {
      this.stream = await navigator.mediaDevices.getUserMedia({
        audio: { channelCount: 1, echoCancellation: true },
      });
    } catch {
      throw new VoiceInputError(
        "permission",
        "Microphone permission denied or unavailable",
      );
    }
    // 授权对话框可能开着好一会儿,期间用户已经取消了。此时再建图就会
    // 留下一个「界面显示没在录、麦克风却被占着」的实例。
    if (generation !== this.generation) {
      await this.teardown();
      return;
    }

    try {
      // 按设备原生速率建 AudioContext,采完在 finish() 里重采样到 16k。
      //
      // 不要在这里指定 sampleRate: 16000。WebKit 的 MediaStreamAudioSourceNode
      // 不会替你把轨道重采样到 context 的速率,速率对不上时它既不报错也不
      // 抛异常,onaudioprocess 照常回调,但送出来的每个采样都是 0——录出来
      // 是一段完美的静音,一路上传到 ASR 才被告知 "no valid speech in audio"。
      // 麦克风轨道普遍是 44.1k/48k,于是打包后的 macOS 版必然踩中;Chromium
      // 会自动重采样,所以浏览器里调试永远看不到这个问题。
      this.context = new Ctor();
      this.source = this.context.createMediaStreamSource(this.stream);
      // AudioWorklet 需要额外的模块 URL,在 Tauri 的 CSP 下容易踩坑;
      // 一段两分钟的语音用 ScriptProcessor 足够,且各端都支持。
      this.processor = this.context.createScriptProcessor(BUFFER_SIZE, 1, 1);
      this.processor.onaudioprocess = (event) => {
        if (!this.active) return;
        // 缓冲区会被复用,必须拷贝一份留存。
        const copied = new Float32Array(event.inputBuffer.getChannelData(0));
        this.chunks.push(copied);
        this.queueNative(copied);
        // 收尾时在等这一块:停止那一刻还有小半块音频卡在管线里。
        const flush = this.pendingFlush;
        if (flush) {
          this.pendingFlush = null;
          flush();
        }
      };
      // ScriptProcessor 不接到目的地就不会回调;经 gain=0 静音,
      // 否则麦克风会从扬声器原样放出来形成回授。
      this.silence = this.context.createGain();
      this.silence.gain.value = 0;
      this.source.connect(this.processor);
      this.processor.connect(this.silence);
      this.silence.connect(this.context.destination);
      // WebKit 新建的 AudioContext 可能是 suspended,而我们是在
      // await getUserMedia 之后建的,已经脱离了点击那一拍的同步上下文,
      // 不显式 resume 就一个回调都收不到,录出来是空的。
      if (this.context.state === "suspended") {
        await this.context.resume();
      }
    } catch {
      await this.teardown();
      throw new VoiceInputError("start_failed", "Failed to start capture");
    }

    // resume() 也是个 await,同样可能在这期间被取消。
    if (generation !== this.generation) {
      await this.teardown();
      return;
    }

    this.active = true;
    this.startedAt = Date.now();
    this.maxTimer = setTimeout(() => {
      if (!this.active) return;
      void this.stop()
        .then((file) => onAutoStop?.(file))
        .catch(() => {
          /* auto-stop failures are non-fatal */
        });
    }, MAX_DURATION_MS);
  }

  async stop(): Promise<File> {
    // 手动停止与两分钟自动停止可能同时发生;没有去重的话两边会各自等
    // 尾块、各自清空 chunks,一个拿到音频、另一个拿到「空录音」。
    if (this.stopPromise) return this.stopPromise;
    if (!this.active) {
      await this.teardown();
      throw new VoiceInputError("empty", "Not recording");
    }
    if (this.maxTimer) {
      clearTimeout(this.maxTimer);
      this.maxTimer = null;
    }
    this.stopPromise = this.finish();
    try {
      return await this.stopPromise;
    } finally {
      this.stopPromise = null;
    }
  }

  private queueNative(chunk: Float32Array): void {
    if (!this.onPcm || !this.context) return;
    this.pendingNative.push(chunk);
    this.pendingNativeFrames += chunk.length;
    const needed = Math.round((this.context.sampleRate * 200) / 1000);
    if (this.pendingNativeFrames >= needed) this.flushPendingPcm();
  }

  private flushPendingPcm(): void {
    if (!this.onPcm || this.pendingNative.length === 0) return;
    const sampleRate = this.context?.sampleRate ?? TARGET_SAMPLE_RATE;
    const captured = mergeChunks(this.pendingNative);
    this.pendingNative = [];
    this.pendingNativeFrames = 0;
    if (!captured.length) return;
    const samples = resampleTo(captured, sampleRate, TARGET_SAMPLE_RATE);
    this.onPcm(floatToPcm16(samples));
  }

  private async finish(): Promise<File> {
    // 停止那一刻还有小半块音频卡在管线里,ScriptProcessor 只在缓冲区
    // 填满时才回调。这里多等一个回调周期(上限 FLUSH_TIMEOUT_MS)把话尾
    // 收进来——代价是可能多录进最多一个缓冲区的环境音,对识别无害。
    await this.awaitFinalChunk();
    this.flushPendingPcm();
    this.active = false;

    const durationMs = Date.now() - this.startedAt;
    const sampleRate = this.context?.sampleRate ?? TARGET_SAMPLE_RATE;
    const captured = mergeChunks(this.chunks);
    this.chunks = [];
    // 无论后面因为太短/为空抛错,麦克风都必须先放开。
    await this.teardown();

    if (durationMs < MIN_DURATION_MS) {
      throw new VoiceInputError("too_short", "Recording too short");
    }
    if (!captured.length) {
      throw new VoiceInputError("empty", "Empty recording");
    }

    const samples = resampleTo(captured, sampleRate, TARGET_SAMPLE_RATE);
    const wav = encodeWav(samples, TARGET_SAMPLE_RATE);
    return new File([wav], `voice-${Date.now()}.wav`, { type: "audio/wav" });
  }

  /** 等一次 onaudioprocess 把尾块交出来;超时就不等了,宁可少几十毫秒。 */
  private awaitFinalChunk(): Promise<void> {
    if (!this.processor) return Promise.resolve();
    return new Promise<void>((resolve) => {
      const timer = setTimeout(() => {
        this.pendingFlush = null;
        resolve();
      }, FLUSH_TIMEOUT_MS);
      this.pendingFlush = () => {
        clearTimeout(timer);
        resolve();
      };
    });
  }

  cancel(): void {
    // 让仍在 await 中的 start() 认出自己已经作废。
    this.generation += 1;
    this.active = false;
    // 取消时若 stop() 正等着尾块,先把它放行,免得挂住。
    const flush = this.pendingFlush;
    this.pendingFlush = null;
    flush?.();
    if (this.maxTimer) {
      clearTimeout(this.maxTimer);
      this.maxTimer = null;
    }
    this.chunks = [];
    this.pendingNative = [];
    this.pendingNativeFrames = 0;
    this.onPcm = null;
    void this.teardown();
  }

  /** 断开音频图并放开麦克风。可重复调用。 */
  private async teardown(): Promise<void> {
    this.processor?.disconnect();
    if (this.processor) this.processor.onaudioprocess = null;
    this.source?.disconnect();
    this.silence?.disconnect();
    this.processor = null;
    this.source = null;
    this.silence = null;
    if (this.stream) {
      for (const track of this.stream.getTracks()) track.stop();
      this.stream = null;
    }
    const context = this.context;
    this.context = null;
    if (context && context.state !== "closed") {
      try {
        await context.close();
      } catch {
        /* already closing */
      }
    }
  }
}
