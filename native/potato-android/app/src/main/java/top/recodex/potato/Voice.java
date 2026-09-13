package top.recodex.potato;

import android.media.*;
import android.os.*;
import java.util.*;
import okhttp3.*;
import okio.ByteString;
import org.json.*;

final class Voice extends WebSocketListener {
  interface Listener {
    default void ready() {}

    default void limit() {}

    void text(String value, boolean finished);

    void level(double value);

    void error(String issue);
  }

  final Listener listener;
  final Handler main = new Handler(Looper.getMainLooper());
  final Api api;
  AudioRecord recorder;
  WebSocket socket;
  volatile boolean ended, capturing, finishing;
  boolean ready;
  int pendingBytes;
  final ArrayDeque<byte[]> pending = new ArrayDeque<>();
  final Runnable timeout = () -> fail("语音连接超时，可用文字已保留。");

  Voice(Api api, Listener listener) {
    this.api = api;
    this.listener = listener;
  }

  @android.annotation.SuppressLint("MissingPermission")
  void start(String endpoint, String token) throws Exception {
    if (token.isEmpty()) throw new Exception("请先登录支持语音的云端服务。");
    String url = Api.servicePath(endpoint, "/v1/audio/transcriptions");
    int size =
        Math.max(
            6400,
            AudioRecord.getMinBufferSize(
                16000, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT));
    recorder =
        new AudioRecord(
            MediaRecorder.AudioSource.VOICE_RECOGNITION,
            16000,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
            size);
    if (recorder.getState() != AudioRecord.STATE_INITIALIZED) {
      recorder.release();
      throw new Exception("麦克风不可用，请检查权限或其他录音应用。");
    }
    recorder.startRecording();
    capturing = true;
    socket =
        api.client.newWebSocket(
            new Request.Builder().url(url).header("Authorization", "Bearer " + token).build(),
            this);
    main.postDelayed(timeout, 20000);
    new Thread(
            () -> {
              short[] pcm = new short[1600];
              long started = System.currentTimeMillis();
              try {
                while (capturing && !ended) {
                  int n = recorder.read(pcm, 0, pcm.length);
                  if (n < 0) {
                    if (!capturing) break;
                    throw new Exception("录音中断。");
                  }
                  if (n == 0) continue;
                  byte[] bytes = new byte[n * 2];
                  double sum = 0;
                  for (int i = 0; i < n; i++) {
                    bytes[i * 2] = (byte) pcm[i];
                    bytes[i * 2 + 1] = (byte) (pcm[i] >> 8);
                    sum += (double) pcm[i] * pcm[i];
                  }
                  append(bytes);
                  double level = Math.min(1, Math.sqrt(sum / n) / 7000);
                  main.post(
                      () -> {
                        if (!ended) listener.level(level);
                      });
                  if (System.currentTimeMillis() - started >= 60000) {
                    main.post(
                        () -> {
                          listener.limit();
                          finish();
                        });
                    break;
                  }
                }
              } catch (Exception e) {
                fail(e.getMessage());
              } finally {
                try {
                  recorder.stop();
                } catch (Exception ignored) {
                }
                recorder.release();
                synchronized (this) {
                  capturing = false;
                  if (finishing && !ended) sendStop();
                }
              }
            },
            "Potato microphone")
        .start();
  }

  synchronized void append(byte[] bytes) throws Exception {
    if (ended) return;
    if (ready) {
      if (socket.queueSize() + bytes.length > 640000 || !socket.send(ByteString.of(bytes)))
        throw new Exception("语音上传过慢，可用文字已保留。");
    } else {
      if (pendingBytes + bytes.length > 640000) throw new Exception("语音连接较慢，录音缓存已满，请重试。");
      pending.add(bytes);
      pendingBytes += bytes.length;
    }
  }

  @Override
  public synchronized void onMessage(WebSocket ws, String text) {
    if (ended) return;
    try {
      if (text.length() > 1000000) throw new Exception("语音响应过大。");
      JSONObject e = new JSONObject(text);
      String type = e.optString("type"), value = e.optString("text");
      switch (type) {
        case "ready":
          ready = true;
          main.post(
              () -> {
                if (!ended) listener.ready();
              });
          if (!finishing) main.removeCallbacks(timeout);
          while (!pending.isEmpty()) {
            if (!ws.send(ByteString.of(pending.remove()))) throw new Exception("语音发送失败。");
          }
          pendingBytes = 0;
          if (finishing && !capturing) sendStop();
          break;
        case "partial":
          main.post(
              () -> {
                if (!ended) listener.text(value, false);
              });
          break;
        case "final":
          ended = true;
          capturing = false;
          main.removeCallbacks(timeout);
          ws.close(1000, "done");
          main.post(() -> listener.text(value, true));
          break;
        default:
          throw new Exception(e.optString("message", "语音服务返回错误。"));
      }
    } catch (Exception e) {
      fail(e.getMessage());
    }
  }

  synchronized void finish() {
    if (ended || finishing) return;
    finishing = true;
    capturing = false;
    main.removeCallbacks(timeout);
    main.postDelayed(timeout, 25000);
  }

  private void sendStop() {
    if (ready) socket.send("{\"type\":\"stop\"}");
  }

  synchronized void cancel() {
    ended = true;
    capturing = false;
    pending.clear();
    main.removeCallbacks(timeout);
    if (socket != null) socket.cancel();
  }

  void fail(String issue) {
    synchronized (this) {
      if (ended) return;
      cancel();
    }
    main.post(() -> listener.error(issue == null ? "语音连接中断。" : issue));
  }

  @Override
  public void onFailure(WebSocket ws, Throwable t, Response r) {
    fail("语音连接失败，请检查网络和云端登录。");
  }

  @Override
  public void onClosed(WebSocket ws, int code, String reason) {
    if (!ended) fail("语音连接提前结束，可用文字已保留。");
  }
}
