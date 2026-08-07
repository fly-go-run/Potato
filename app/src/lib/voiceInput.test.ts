import { afterEach, describe, expect, it, vi } from "vitest";
import {
  encodeWav,
  resampleTo,
  TARGET_SAMPLE_RATE,
  VoiceInputError,
  VoiceRecorder,
} from "./voiceInput";

describe("resampleTo", () => {
  it("passes through when the rate already matches", () => {
    const input = new Float32Array([0.1, 0.2, 0.3]);
    expect(resampleTo(input, 16_000, 16_000)).toBe(input);
  });

  it("decimates 48k down to the ASR rate", () => {
    // 浏览器常见的是 48k;拿不到 16k 的 AudioContext 时要靠这里补上,
    // 否则送去识别的音频会整体变调。
    const input = new Float32Array(48_000).fill(0.5);
    const output = resampleTo(input, 48_000, TARGET_SAMPLE_RATE);
    expect(output.length).toBe(16_000);
    expect(output[0]).toBeCloseTo(0.5, 5);
  });

  it("keeps a ramp monotonic after resampling", () => {
    const input = Float32Array.from({ length: 900 }, (_, i) => i / 900);
    const output = resampleTo(input, 45_000, 15_000);
    expect(output.length).toBe(300);
    for (let i = 1; i < output.length; i += 1) {
      expect(output[i]!).toBeGreaterThan(output[i - 1]!);
    }
  });
});

describe("encodeWav", () => {
  const read = (buffer: ArrayBuffer, offset: number, length: number) =>
    String.fromCharCode(
      ...new Uint8Array(buffer.slice(offset, offset + length)),
    );

  it("writes a 44-byte 16-bit mono PCM header", () => {
    const buffer = encodeWav(new Float32Array([0, 0]), TARGET_SAMPLE_RATE);
    const view = new DataView(buffer);

    expect(read(buffer, 0, 4)).toBe("RIFF");
    expect(read(buffer, 8, 4)).toBe("WAVE");
    expect(read(buffer, 12, 4)).toBe("fmt ");
    expect(read(buffer, 36, 4)).toBe("data");
    expect(view.getUint16(20, true)).toBe(1); // PCM
    expect(view.getUint16(22, true)).toBe(1); // mono
    expect(view.getUint32(24, true)).toBe(TARGET_SAMPLE_RATE);
    expect(view.getUint16(34, true)).toBe(16); // bit depth
    // 头 44 字节 + 每个采样 2 字节
    expect(buffer.byteLength).toBe(44 + 2 * 2);
    expect(view.getUint32(4, true)).toBe(buffer.byteLength - 8);
    expect(view.getUint32(40, true)).toBe(4);
  });

  it("scales samples to int16 and clamps out-of-range values", () => {
    const buffer = encodeWav(
      new Float32Array([0, 1, -1, 2, -2]),
      TARGET_SAMPLE_RATE,
    );
    const view = new DataView(buffer);
    expect(view.getInt16(44, true)).toBe(0);
    expect(view.getInt16(46, true)).toBe(32767);
    expect(view.getInt16(48, true)).toBe(-32767);
    // 超范围的输入必须被夹住,否则回绕成反相的爆音。
    expect(view.getInt16(50, true)).toBe(32767);
    expect(view.getInt16(52, true)).toBe(-32767);
  });
});

/* ---------------------------------------------------------------------- *
 * 录音器生命周期。vitest 跑在 node 环境,这里搭一套最小的 Web Audio 替身,
 * 覆盖 codex review 点名的几条:尾块、resume、并发停止、失败清理。
 * ---------------------------------------------------------------------- */

class FakeTrack {
  stopped = false;
  stop() {
    this.stopped = true;
  }
}

class FakeStream {
  constructor(readonly tracks: FakeTrack[] = [new FakeTrack()]) {}
  getTracks() {
    return this.tracks;
  }
}

class FakeProcessor {
  onaudioprocess: ((event: unknown) => void) | null = null;
  connect() {}
  disconnect() {}
  /** 模拟一次采集回调。 */
  emit(fill: number, frames = 8) {
    const data = new Float32Array(frames).fill(fill);
    this.onaudioprocess?.({ inputBuffer: { getChannelData: () => data } });
  }
}

class FakeContext {
  static last: FakeContext | null = null;
  static startSuspended = false;
  state: "running" | "suspended" | "closed" = "running";
  sampleRate: number;
  destination = {};
  processor = new FakeProcessor();
  resumed = 0;
  closed = 0;

  constructor(options?: { sampleRate?: number }) {
    this.sampleRate = options?.sampleRate ?? 48_000;
    if (FakeContext.startSuspended) this.state = "suspended";
    FakeContext.last = this;
  }
  createMediaStreamSource() {
    return { connect() {}, disconnect() {} };
  }
  createScriptProcessor() {
    return this.processor;
  }
  createGain() {
    return { gain: { value: 1 }, connect() {}, disconnect() {} };
  }
  async resume() {
    this.resumed += 1;
    this.state = "running";
  }
  async close() {
    this.closed += 1;
    this.state = "closed";
  }
}

function installWebAudio(getUserMedia?: () => Promise<unknown>) {
  const stream = new FakeStream();
  vi.stubGlobal("window", { AudioContext: FakeContext });
  vi.stubGlobal("navigator", {
    mediaDevices: {
      getUserMedia: getUserMedia ?? (() => Promise.resolve(stream as never)),
    },
  });
  return stream;
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  FakeContext.last = null;
  FakeContext.startSuspended = false;
});

describe("VoiceRecorder lifecycle", () => {
  it("keeps the trailing buffer that arrives after stop is requested", async () => {
    const stream = installWebAudio();
    vi.useFakeTimers();
    const recorder = new VoiceRecorder();
    await recorder.start();
    const context = FakeContext.last!;
    context.processor.emit(0.5);
    vi.advanceTimersByTime(1_000); // 越过 MIN_DURATION_MS

    // stop() 先等一次回调:停止那一刻卡在管线里的话尾必须进 wav。
    const stopping = recorder.stop();
    context.processor.emit(0.25);
    const file = await stopping;
    vi.useRealTimers();

    // context 是设备原生的 48k(录音器不再强制 16k,那会让 WebKit 送出
    // 整段静音),48k→16k 降采样 3:1:两块各 8 帧 = 16 个采样 → 5 个,
    // 加 44 字节头。若尾块被丢掉,只剩 8 帧 → 2 个采样 = 48 字节。
    expect(file.name).toMatch(/^voice-\d+\.wav$/);
    expect(file.type).toBe("audio/wav");
    expect(file.size).toBe(44 + 5 * 2);
    expect(stream.tracks[0]!.stopped).toBe(true);
    expect(context.closed).toBe(1);
  });

  it("resumes a suspended context so callbacks actually fire", async () => {
    installWebAudio();
    FakeContext.startSuspended = true;
    const recorder = new VoiceRecorder();

    await recorder.start();

    // WebKit 在 await getUserMedia 之后新建的 context 可能是 suspended,
    // 不 resume 就一个回调都收不到,录出来是空的。
    expect(FakeContext.last!.resumed).toBe(1);
    expect(FakeContext.last!.state).toBe("running");
    recorder.cancel();
  });

  it("releases the microphone when start fails midway", async () => {
    const stream = installWebAudio();
    vi.spyOn(FakeContext.prototype, "createScriptProcessor").mockImplementation(
      () => {
        throw new Error("no processor");
      },
    );
    const recorder = new VoiceRecorder();

    await expect(recorder.start()).rejects.toBeInstanceOf(VoiceInputError);
    expect(stream.tracks[0]!.stopped).toBe(true);
  });

  it("cancel releases everything and unblocks a pending stop", async () => {
    const stream = installWebAudio();
    const recorder = new VoiceRecorder();
    await recorder.start();
    FakeContext.last!.processor.emit(0.5);

    const stopping = recorder.stop();
    recorder.cancel(); // 不会再有回调,cancel 必须把 stop 放行

    await expect(stopping).rejects.toBeInstanceOf(VoiceInputError);
    expect(stream.tracks[0]!.stopped).toBe(true);
    expect(recorder.recording).toBe(false);
  });

  it("reports an unsupported environment instead of a raw error", async () => {
    vi.stubGlobal("window", {});
    vi.stubGlobal("navigator", {});
    const recorder = new VoiceRecorder();

    await expect(recorder.start()).rejects.toMatchObject({
      code: "unsupported",
    });
  });
});

describe("VoiceRecorder concurrency", () => {
  it("dedupes a manual stop racing the auto-stop timer", async () => {
    installWebAudio();
    vi.useFakeTimers();
    const recorder = new VoiceRecorder();
    await recorder.start();
    const context = FakeContext.last!;
    context.processor.emit(0.5);
    vi.advanceTimersByTime(1_000);

    // 两次 stop() 必须共用同一个 promise,否则两边各自清空 chunks,
    // 一个拿到音频、另一个拿到「空录音」。
    const first = recorder.stop();
    const second = recorder.stop();
    context.processor.emit(0.25);
    const [a, b] = await Promise.all([first, second]);
    vi.useRealTimers();

    expect(a).toBe(b);
    // 同上:16 个 48k 采样降到 16k 后是 5 个。
    expect(a.size).toBe(44 + 5 * 2);
  });

  it("does not arm a recording that was cancelled mid-permission", async () => {
    let release: ((stream: unknown) => void) | undefined;
    const stream = new FakeStream();
    installWebAudio(
      () =>
        new Promise((resolve) => {
          release = resolve;
        }),
    );
    const recorder = new VoiceRecorder();

    const starting = recorder.start();
    recorder.cancel(); // 授权对话框还开着的时候就取消
    release!(stream);
    await starting;

    // 不能留下「界面显示没在录、麦克风却被占着」的状态。
    expect(recorder.recording).toBe(false);
    expect(stream.tracks[0]!.stopped).toBe(true);
  });
});
