package top.recodex.potato;

import static org.junit.Assert.*;

import java.io.IOException;
import java.util.*;
import java.util.concurrent.TimeUnit;
import okhttp3.*;
import okhttp3.mockwebserver.*;
import okio.Buffer;
import org.json.*;
import org.junit.Test;

public class ApiTest {
  @Test
  public void chineseReasoningSearchAndDone() throws Exception {
    Buffer b =
        new Buffer()
            .writeUtf8(
                ": heartbeat\r\n\r\n"
                    + "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"想一想\"}}]}\r\n\r\n"
                    + "data: {\"choices\":[{\"delta\":{\"content\":\"你好，世界🌱\"}}]}\n\n"
                    + "data:"
                    + " {\"potato_search\":{\"id\":\"one\",\"query\":\"天气\",\"results\":[]}}\n\n"
                    + "data: [DONE]\n\n");
    List<JSONObject> events = new ArrayList<>();
    Api.readSse(b, events::add);
    assertEquals(3, events.size());
    assertEquals(
        "你好，世界🌱",
        events
            .get(1)
            .getJSONArray("choices")
            .getJSONObject(0)
            .getJSONObject("delta")
            .getString("content"));
    assertTrue(events.get(2).has("potato_search"));
  }

  @Test
  public void missingTerminalPreservesPartialAndFails() throws Exception {
    List<JSONObject> events = new ArrayList<>();
    assertThrows(
        IOException.class,
        () ->
            Api.readSse(
                new Buffer()
                    .writeUtf8("data: {\"choices\":[{\"delta\":{\"content\":\"部分\"}}]}\n\n"),
                events::add));
    assertEquals(1, events.size());
  }

  @Test
  public void multilineData() throws Exception {
    List<JSONObject> events = new ArrayList<>();
    Api.readSse(
        new Buffer()
            .writeUtf8(
                "data: {\"choices\":\n"
                    + "data: [{\"delta\":{\"content\":\"好\"}}]}\n\n"
                    + "data: [DONE]\n\n"),
        events::add);
    assertEquals(1, events.size());
  }

  @Test
  public void outputLimitIsNotSuccess() {
    assertThrows(
        IOException.class,
        () ->
            Api.readSse(
                new Buffer()
                    .writeUtf8(
                        "data:"
                            + " {\"choices\":[{\"delta\":{\"content\":\"部分\"},\"finish_reason\":\"length\"}]}\n\n"
                            + "data: [DONE]\n\n"),
                e -> {}));
  }

  @Test
  public void errorFrameIsNotSuccess() {
    assertThrows(
        IOException.class,
        () ->
            Api.readSse(
                new Buffer().writeUtf8("data: {\"error\":{\"message\":\"no\"}}\n\n"), e -> {}));
  }

  @Test
  public void rejectOversizedLine() {
    assertThrows(
        IOException.class,
        () ->
            Api.readSse(
                new Buffer().writeUtf8("data: " + "x".repeat(1_000_001) + "\n\n"), e -> {}));
  }

  @Test
  public void endpointsRejectCredentialLeakAndInsecureSchemes() throws Exception {
    for (String value :
        List.of(
            "http://example.com/v1/chat/completions",
            "https://user:password@example.com/v1/chat/completions",
            "https://example.com/v1/chat/completions?token=secret",
            "https://example.com/v1/chat/completions#fragment",
            "file:///tmp/file")) assertThrows(IOException.class, () -> Api.url(value));
    assertEquals("api.example.com", Api.url("https://api.example.com/v1/chat/completions").host());
  }

  @Test
  public void servicePathRetainsPrefixButNotChatPath() throws Exception {
    assertEquals(
        "https://api.example.com/proxy/v1/audio/transcriptions",
        Api.servicePath(
            "https://api.example.com/proxy/v1/chat/completions", "/v1/audio/transcriptions"));
    assertThrows(
        IOException.class,
        () -> Api.servicePath("https://example.com/messages", "/v1/sandbox/run"));
  }

  @Test
  public void actualHttpStreamWithFragmentedUtf8() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setHeader("Content-Type", "text/event-stream")
              .setBody(
                  "data: {\"choices\":[{\"delta\":{\"content\":\"中文🌱\"}}]}\n\ndata: [DONE]\n\n")
              .throttleBody(1, 1, TimeUnit.MILLISECONDS));
      Api api = new Api();
      List<JSONObject> events = new ArrayList<>();
      api.stream(
          api.client.newCall(new Request.Builder().url(server.url("/chat")).build()), events::add);
      assertEquals(
          "中文🌱",
          events
              .get(0)
              .getJSONArray("choices")
              .getJSONObject(0)
              .getJSONObject("delta")
              .getString("content"));
    }
  }

  @Test
  public void redirectsNeverForwardAuthorization() throws Exception {
    try (MockWebServer source = new MockWebServer();
        MockWebServer target = new MockWebServer()) {
      target.start();
      source.enqueue(
          new MockResponse().setResponseCode(302).addHeader("Location", target.url("/stolen")));
      Api api = new Api();
      assertThrows(
          IOException.class,
          () ->
              api.stream(
                  api.client.newCall(
                      new Request.Builder()
                          .url(source.url("/"))
                          .header("Authorization", "Bearer synthetic-test-token")
                          .build()),
                  e -> {}));
      assertEquals(0, target.getRequestCount());
    }
  }

  @Test
  public void nonSseResponseRejected() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setHeader("Content-Type", "text/html").setBody("login"));
      Api api = new Api();
      assertThrows(
          IOException.class,
          () ->
              api.stream(
                  api.client.newCall(new Request.Builder().url(server.url("/")).build()), e -> {}));
    }
  }

  @Test
  public void cancellationUnblocksStream() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE));
      Api api = new Api();
      Call call = api.client.newCall(new Request.Builder().url(server.url("/")).build());
      Thread cancel =
          new Thread(
              () -> {
                try {
                  Thread.sleep(100);
                } catch (InterruptedException ignored) {
                }
                call.cancel();
              });
      cancel.start();
      assertThrows(IOException.class, () -> api.stream(call, e -> {}));
      cancel.join();
      assertTrue(call.isCanceled());
    }
  }

  @Test
  public void requestBudgetRejectedBeforeNetwork() {
    Api api = new Api();
    assertThrows(
        IOException.class,
        () ->
            api.streamCall(
                "https://example.com/v1/chat/completions",
                "",
                Json.obj("text", "中".repeat(1_500_000))));
  }
}
