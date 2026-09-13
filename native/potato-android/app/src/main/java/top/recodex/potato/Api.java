package top.recodex.potato;

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.TimeUnit;
import okhttp3.*;
import okio.BufferedSource;
import org.json.*;

final class Api {
  static final MediaType JSON = MediaType.get("application/json; charset=utf-8");
  final OkHttpClient client;

  Api() {
    this(
        new OkHttpClient.Builder()
            .connectTimeout(20, TimeUnit.SECONDS)
            .readTimeout(90, TimeUnit.SECONDS)
            .callTimeout(300, TimeUnit.SECONDS)
            .followRedirects(false)
            .followSslRedirects(false)
            .retryOnConnectionFailure(false)
            .build());
  }

  Api(OkHttpClient client) {
    this.client = client;
  }

  static HttpUrl url(String value) throws IOException {
    HttpUrl u = HttpUrl.parse(value);
    if (u == null
        || !u.isHttps()
        || !u.username().isEmpty()
        || !u.password().isEmpty()
        || u.fragment() != null
        || u.query() != null) throw new IOException("请输入完整、安全的 HTTPS 接口地址。");
    return u;
  }

  Request request(String url, String token, JSONObject body) throws IOException {
    Request.Builder b = new Request.Builder().url(url(url)).header("Accept", "application/json");
    if (!token.isEmpty()) b.header("Authorization", "Bearer " + token);
    if (body != null) b.post(RequestBody.create(body.toString(), JSON));
    return b.build();
  }

  JSONObject json(String url, String token, JSONObject body) throws Exception {
    try (Response r = client.newCall(request(url, token, body)).execute()) {
      check(r);
      BufferedSource source = r.body().source();
      if (source.request(4_200_001)) throw new IOException("服务响应过大。");
      return new JSONObject(source.readUtf8());
    }
  }

  static void check(Response r) throws IOException {
    if (r.isSuccessful() && r.body() != null) return;
    String issue =
        switch (r.code()) {
          case 401, 403 -> "登录已失效或此账号未获授权，请在设置中重新登录。";
          case 429 -> "请求频繁或额度不足，请稍后再试。";
          case 404 -> "找不到接口或模型，请检查服务配置。";
          case 503 -> "服务暂不可用或未配置此能力。";
          default -> "服务请求失败（" + r.code() + "），请稍后重试。";
        };
    throw new HttpFailure(r.code(), issue);
  }

  static final class HttpFailure extends IOException {
    final int status;

    HttpFailure(int status, String message) {
      super(message);
      this.status = status;
    }
  }

  interface Events {
    void event(JSONObject event) throws Exception;
  }

  static void readSse(BufferedSource source, Events callback) throws Exception {
    StringBuilder data = new StringBuilder();
    int total = 0;
    while (!source.exhausted()) {
      String line = source.readUtf8LineStrict(1_000_000);
      if (line.isEmpty()) {
        if (data.length() == 0) continue;
        String value = data.toString();
        data.setLength(0);
        if (value.equals("[DONE]")) return;
        JSONObject event = new JSONObject(value);
        if (event.has("error")) throw new IOException("模型服务返回错误，请稍后重试。");
        total += value.length();
        if (total > 4_000_000) throw new IOException("回复过长，已保留收到的内容。");
        callback.event(event);
        JSONArray choices = event.optJSONArray("choices");
        if (choices != null
            && choices.length() > 0
            && choices.optJSONObject(0).optString("finish_reason").equals("length"))
          throw new IOException("回复达到输出上限，已保留已有内容。");
      } else if (line.startsWith("data:")) {
        String value = line.substring(5);
        if (value.startsWith(" ")) value = value.substring(1);
        if (data.length() > 0) data.append('\n');
        data.append(value);
        if (data.length() > 1_000_000) throw new IOException("服务器事件过大。");
      }
    }
    throw new IOException("连接中断，已保留收到的内容。可以重试。");
  }

  Call streamCall(String endpoint, String token, JSONObject body) throws IOException {
    if (body.toString().getBytes(StandardCharsets.UTF_8).length > 4 * 1024 * 1024)
      throw new IOException("对话超过 4 MB，请减少附件或新建对话。");
    return client.newCall(
        request(endpoint, token, body).newBuilder().header("Accept", "text/event-stream").build());
  }

  void stream(Call call, Events callback) throws Exception {
    try (Response response = call.execute()) {
      check(response);
      if (!response
          .header("Content-Type", "")
          .toLowerCase(java.util.Locale.ROOT)
          .contains("text/event-stream")) throw new IOException("服务未返回兼容的流式响应。");
      readSse(response.body().source(), callback);
    }
  }

  static String servicePath(String endpoint, String path) throws IOException {
    HttpUrl u = url(endpoint);
    String suffix = "/v1/chat/completions";
    if (!u.encodedPath().endsWith(suffix)) throw new IOException("此连接不支持 Potato 云端能力。");
    return u.newBuilder()
        .encodedPath(
            u.encodedPath().substring(0, u.encodedPath().length() - suffix.length()) + path)
        .build()
        .toString();
  }
}
