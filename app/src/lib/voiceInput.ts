/** Browser microphone capture via MediaRecorder for composer STT. */

export type VoiceInputErrorCode =
  | "unsupported"
  | "permission"
  | "empty"
  | "too_short"
  | "start_failed";

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

function pickMimeType(): string | undefined {
  if (typeof MediaRecorder === "undefined") return undefined;
  const candidates = [
    "audio/webm;codecs=opus",
    "audio/webm",
    "audio/ogg;codecs=opus",
    "audio/mp4",
  ];
  for (const type of candidates) {
    if (MediaRecorder.isTypeSupported(type)) return type;
  }
  return undefined;
}

function extensionForMime(mime: string): string {
  if (mime.includes("ogg")) return "ogg";
  if (mime.includes("mp4") || mime.includes("m4a")) return "m4a";
  if (mime.includes("wav")) return "wav";
  return "webm";
}

export class VoiceRecorder {
  private mediaRecorder: MediaRecorder | null = null;
  private stream: MediaStream | null = null;
  private chunks: BlobPart[] = [];
  private startedAt = 0;
  private maxTimer: ReturnType<typeof setTimeout> | null = null;
  private mimeType = "";
  private onAutoStop: ((file: File) => void) | null = null;
  private stopPromise: Promise<File> | null = null;

  get recording(): boolean {
    return this.mediaRecorder?.state === "recording";
  }

  async start(onAutoStop?: (file: File) => void): Promise<void> {
    if (
      typeof navigator === "undefined" ||
      !navigator.mediaDevices?.getUserMedia ||
      typeof MediaRecorder === "undefined"
    ) {
      throw new VoiceInputError(
        "unsupported",
        "MediaRecorder is not available in this environment",
      );
    }

    this.onAutoStop = onAutoStop ?? null;
    this.chunks = [];
    const mimeType = pickMimeType();
    this.mimeType = mimeType ?? "";

    try {
      this.stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch {
      throw new VoiceInputError(
        "permission",
        "Microphone permission denied or unavailable",
      );
    }

    try {
      this.mediaRecorder = mimeType
        ? new MediaRecorder(this.stream, { mimeType })
        : new MediaRecorder(this.stream);
      this.mimeType = this.mediaRecorder.mimeType || mimeType || "audio/webm";
    } catch {
      this.cleanupStream();
      throw new VoiceInputError("start_failed", "Failed to start MediaRecorder");
    }

    this.mediaRecorder.ondataavailable = (event) => {
      if (event.data.size > 0) this.chunks.push(event.data);
    };

    this.startedAt = Date.now();
    this.mediaRecorder.start(250);
    this.maxTimer = setTimeout(() => {
      if (!this.recording) return;
      void this
        .stop()
        .then((file) => this.onAutoStop?.(file))
        .catch(() => {
          /* auto-stop failures are non-fatal */
        });
    }, MAX_DURATION_MS);
  }

  async stop(): Promise<File> {
    if (this.stopPromise) return this.stopPromise;

    const recorder = this.mediaRecorder;
    if (!recorder || recorder.state === "inactive") {
      this.cleanupStream();
      throw new VoiceInputError("empty", "Not recording");
    }

    if (this.maxTimer) {
      clearTimeout(this.maxTimer);
      this.maxTimer = null;
    }

    const durationMs = Date.now() - this.startedAt;
    this.stopPromise = (async () => {
      const blob = await new Promise<Blob>((resolve, reject) => {
        recorder.onstop = () => {
          resolve(
            new Blob(this.chunks, { type: this.mimeType || "audio/webm" }),
          );
        };
        recorder.onerror = () => {
          reject(new VoiceInputError("start_failed", "Recording failed"));
        };
        try {
          recorder.stop();
        } catch (error) {
          reject(error);
        }
      });

      this.cleanupStream();
      this.mediaRecorder = null;
      this.chunks = [];

      if (durationMs < MIN_DURATION_MS) {
        throw new VoiceInputError("too_short", "Recording too short");
      }
      if (!blob.size) {
        throw new VoiceInputError("empty", "Empty recording");
      }

      const ext = extensionForMime(this.mimeType || blob.type);
      return new File([blob], `voice-${Date.now()}.${ext}`, {
        type: blob.type || this.mimeType || "audio/webm",
      });
    })();

    try {
      return await this.stopPromise;
    } finally {
      this.stopPromise = null;
    }
  }

  cancel(): void {
    if (this.maxTimer) {
      clearTimeout(this.maxTimer);
      this.maxTimer = null;
    }
    this.stopPromise = null;
    try {
      if (this.mediaRecorder && this.mediaRecorder.state !== "inactive") {
        this.mediaRecorder.onstop = null;
        this.mediaRecorder.stop();
      }
    } catch {
      /* ignore */
    }
    this.mediaRecorder = null;
    this.chunks = [];
    this.cleanupStream();
  }

  private cleanupStream(): void {
    if (this.stream) {
      for (const track of this.stream.getTracks()) track.stop();
      this.stream = null;
    }
  }
}
