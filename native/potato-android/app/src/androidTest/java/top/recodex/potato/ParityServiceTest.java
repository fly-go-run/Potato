package top.recodex.potato;

import static org.junit.Assert.*;

import androidx.test.core.app.ActivityScenario;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import java.io.File;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;
import okhttp3.*;
import okhttp3.mockwebserver.*;
import okhttp3.tls.*;
import org.json.*;
import org.junit.*;
import org.junit.runner.RunWith;

@RunWith(AndroidJUnit4.class)
public class ParityServiceTest {
  ActivityScenario<MainActivity> scenario;
  MockWebServer server;
  OkHttpClient client;
  MainActivity activity;

  @Before
  public void before() throws Exception {
    var context = InstrumentationRegistry.getInstrumentation().getTargetContext();
    for (String f : new String[] {"workspace.json", "workspace.json.bak", "workspace.json.new"})
      new File(context.getFilesDir(), f).delete();
    HeldCertificate certificate =
        new HeldCertificate.Builder()
            .addSubjectAlternativeName("localhost")
            .commonName("localhost")
            .build();
    HandshakeCertificates tlsServer =
        new HandshakeCertificates.Builder().heldCertificate(certificate).build();
    HandshakeCertificates tlsClient =
        new HandshakeCertificates.Builder()
            .addTrustedCertificate(certificate.certificate())
            .build();
    server = new MockWebServer();
    server.useHttps(tlsServer.sslSocketFactory(), false);
    server.start();
    client =
        new Api()
            .client
            .newBuilder()
            .sslSocketFactory(tlsClient.sslSocketFactory(), tlsClient.trustManager())
            .build();
    String endpoint = server.url("/v1/chat/completions").toString();
    scenario = ActivityScenario.launch(MainActivity.class);
    scenario.onActivity(
        a -> {
          Json.put(a.store.root, "chats", new JSONArray());
          a.store.newChat();
          a.showChat();
        });
    scenario.onActivity(
        a -> {
          activity = a;
          a.api = new Api(client);
          Json.put(a.store.settings(), "demo", false);
          Json.put(a.store.settings(), "model", "synthetic-model");
          Json.put(a.store.settings(), "endpoint", endpoint);
          a.persist();
        });
  }

  @After
  public void after() throws Exception {
    scenario.close();
    server.shutdown();
  }

  void waitFor(java.util.function.Predicate<MainActivity> predicate) throws Exception {
    long deadline = System.currentTimeMillis() + 8000;
    AtomicBoolean ready = new AtomicBoolean();
    while (System.currentTimeMillis() < deadline) {
      scenario.onActivity(a -> ready.set(predicate.test(a)));
      if (ready.get()) return;
      Thread.sleep(50);
    }
    fail("Timed out awaiting native state");
  }

  MockResponse json(JSONObject body) {
    return new MockResponse()
        .setHeader("Content-Type", "application/json")
        .setBody(body.toString());
  }

  JSONObject recallStatus() {
    return Json.obj(
        "scope", "synthetic-account", "entries", Json.obj(), "memories", new JSONArray());
  }

  @Test
  public void historySyncSendsOnlyCompletedTextAndAcknowledgesRevision() throws Exception {
    JSONObject c =
        Json.obj(
            "id",
            Json.id(),
            "title",
            "合成历史",
            "created",
            1700000000000L,
            "messages",
            new JSONArray()
                .put(
                    Json.obj(
                        "id",
                        Json.id(),
                        "role",
                        "user",
                        "text",
                        "用户明确说的内容",
                        "state",
                        "complete",
                        "attachments",
                        new JSONArray().put(Json.obj("file", "private.jpg"))))
                .put(
                    Json.obj(
                        "id",
                        Json.id(),
                        "role",
                        "assistant",
                        "text",
                        "未完成的回复",
                        "state",
                        "interrupted")));
    server.enqueue(json(recallStatus()));
    server.enqueue(json(Json.obj("revision", "receipt-revision")));
    server.enqueue(json(recallStatus()));
    RecallScreen.synchronize(
        activity,
        Json.copy(activity.store.settings()),
        "synthetic-token",
        new JSONArray().put(c),
        false);
    assertEquals("/v1/recall/status", server.takeRequest(2, TimeUnit.SECONDS).getPath());
    RecordedRequest sync = server.takeRequest(2, TimeUnit.SECONDS);
    assertEquals("/v1/recall/sync", sync.getPath());
    JSONObject envelope = new JSONObject(sync.getBody().readUtf8()),
        payload = new JSONObject(envelope.getString("content"));
    assertTrue(envelope.isNull("base"));
    assertEquals(1, payload.getJSONArray("messages").length());
    assertFalse(envelope.toString().contains("private.jpg"));
    assertFalse(envelope.toString().contains("未完成的回复"));
    server.takeRequest(2, TimeUnit.SECONDS);
    server.enqueue(
        json(
            Json.obj(
                "scope",
                "synthetic-account",
                "entries",
                Json.obj(
                    c.optString("id"),
                    Json.obj("revision", "changed-on-another-device", "excluded", false)),
                "memories",
                new JSONArray())));
    server.enqueue(new MockResponse().setResponseCode(409).setBody("conflict"));
    try {
      RecallScreen.synchronize(
          activity,
          Json.copy(activity.store.settings()),
          "synthetic-token",
          new JSONArray().put(c),
          false);
      fail("must surface conflict");
    } catch (Api.HttpFailure e) {
      assertEquals(409, e.status);
    }
    server.takeRequest(2, TimeUnit.SECONDS);
    JSONObject retried =
        new JSONObject(server.takeRequest(2, TimeUnit.SECONDS).getBody().readUtf8());
    assertEquals("receipt-revision", retried.getString("base"));
    assertEquals(5, server.getRequestCount());
  }

  @Test
  public void optOutOnlySyncsExclusionsAndKeepsMessagesOffWire() throws Exception {
    JSONObject excluded =
        Json.obj(
            "id",
            Json.id(),
            "title",
            "排除的对话",
            "recallExcluded",
            true,
            "messages",
            new JSONArray().put(Json.obj("text", "不应上传的内容", "state", "complete")));
    JSONObject visible =
        Json.obj(
            "id",
            Json.id(),
            "title",
            "未排除",
            "messages",
            new JSONArray().put(Json.obj("text", "关闭后不应新增", "state", "complete")));
    server.enqueue(
        json(
            Json.obj(
                "scope",
                "cleanup-account",
                "entries",
                Json.obj(excluded.optString("id"), Json.obj("revision", "old", "excluded", false)),
                "memories",
                new JSONArray())));
    server.enqueue(json(Json.obj("revision", "excluded-revision")));
    server.enqueue(json(recallStatus()));
    RecallScreen.synchronize(
        activity,
        Json.copy(activity.store.settings()),
        "synthetic-token",
        new JSONArray().put(visible).put(excluded),
        true);
    server.takeRequest(2, TimeUnit.SECONDS);
    JSONObject payload =
        new JSONObject(
            new JSONObject(server.takeRequest(2, TimeUnit.SECONDS).getBody().readUtf8())
                .getString("content"));
    assertTrue(payload.getBoolean("excluded"));
    assertEquals(0, payload.getJSONArray("messages").length());
    assertEquals(3, server.getRequestCount());
  }

  @Test
  public void finalEmptyVoiceKeepsPartialAndNeverSends() throws Exception {
    final okhttp3.WebSocket[] socket = {null};
    server.enqueue(
        new MockResponse()
            .withWebSocketUpgrade(
                new WebSocketListener() {
                  @Override
                  public void onOpen(WebSocket ws, Response response) {
                    socket[0] = ws;
                    ws.send("{\"type\":\"ready\"}");
                    ws.send("{\"type\":\"partial\",\"text\":\"语音文字\"}");
                  }

                  @Override
                  public void onMessage(WebSocket ws, String message) {
                    if (message.contains("stop")) {
                      ws.send("{\"type\":\"final\",\"text\":\"\"}");
                      ws.close(1000, "fixture done");
                    }
                  }
                }));
    InstrumentationRegistry.getInstrumentation()
        .getUiAutomation()
        .grantRuntimePermission(
            activity.getPackageName(), android.Manifest.permission.RECORD_AUDIO);
    scenario.onActivity(
        a -> {
          try {
            a.store.secret("custom:" + a.store.settings().optString("endpoint"), "voice-fixture");
          } catch (Exception e) {
            throw new AssertionError(e);
          }
          a.input.setText("原稿前后");
          a.input.setSelection(2);
          a.startVoice();
        });
    waitFor(a -> a.voiceTranscript.equals("语音文字"));
    ParityFlowTest.capture("13-voice");
    scenario.onActivity(
        a -> {
          assertEquals("原稿语音文字前后", a.input.getText().toString());
          a.finishVoice(true);
        });
    waitFor(a -> a.voice == null);
    scenario.onActivity(
        a -> {
          assertEquals("原稿语音文字前后", a.input.getText().toString());
          assertEquals(0, Json.array(a.store.current(), "messages").length());
        });
    assertEquals(1, server.getRequestCount());
  }

  @Test
  public void remoteStateSeparatesReasoningToolReplyAndFailure() {
    scenario.onActivity(
        a -> {
          RemoteScreen r = a.remote;
          r.visible = true;
          r.device = Json.obj("id", "test", "name", "合成电脑", "relay", "https://example.invalid");
          r.chat = Json.obj("id", "test-task");
          r.confirmed = System.currentTimeMillis();
          r.snapshot =
              Json.obj(
                  "status",
                  "running",
                  "messages",
                  new JSONArray().put(Json.obj("kind", "reasoning", "text", "思考中")));
          assertEquals("正在思考", r.remoteStatus());
          Json.array(r.snapshot, "messages")
              .put(Json.obj("kind", "function_call", "text", "exec_command"));
          assertEquals("执行命令", r.remoteStatus());
          Json.array(r.snapshot, "messages").put(Json.obj("kind", "message", "text", "回答内容"));
          assertEquals("正在回复", r.remoteStatus());
          Json.put(r.snapshot, "status", "idle");
          Json.put(r.snapshot, "outcome", Json.obj("status", "failed"));
          assertEquals("本轮任务失败", r.remoteStatus());
          r.confirmed = 0;
          assertEquals("正在确认任务状态…", r.remoteStatus());
          r.visible = false;
        });
  }
}
