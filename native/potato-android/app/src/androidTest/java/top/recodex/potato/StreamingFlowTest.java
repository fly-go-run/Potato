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
public class StreamingFlowTest {
  ActivityScenario<MainActivity> scenario;
  MockWebServer server;
  OkHttpClient client;

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

  @Test
  public void visibleChineseStreamAndStopKeepPartialDraft() throws Exception {
    String event = "data: {\"choices\":[{\"delta\":{\"content\":\"你好，中文流式🌱\"}}]}\n\n";
    server.enqueue(
        new MockResponse()
            .setHeader("Content-Type", "text/event-stream")
            .setBody(event.repeat(30) + "data: [DONE]\n\n")
            .throttleBody(200, 200, TimeUnit.MILLISECONDS));
    scenario.onActivity(
        a -> {
          a.input.setText("合成问题");
          a.send();
        });
    waitFor(a -> a.generating != null && a.generating.optString("text").contains("中文"));
    Thread.sleep(150);
    scenario.onActivity(
        a -> {
          assertNotNull(a.generating);
          assertTrue(a.rows.getChildCount() >= 2);
          a.input.setText("未发送的后续草稿");
          a.stopGeneration("用户停止");
          JSONObject m = Json.array(a.store.current(), "messages").optJSONObject(1);
          assertTrue(m.optString("text").contains("中文"));
          assertEquals("interrupted", m.optString("state"));
          assertEquals("未发送的后续草稿", a.store.current().optString("input"));
        });
    RecordedRequest request = server.takeRequest(2, TimeUnit.SECONDS);
    assertNotNull(request);
    assertEquals("/v1/chat/completions", request.getPath());
    assertTrue(request.getBody().readUtf8().contains("合成问题"));
  }

  @Test
  public void retryUsesOriginalModelAndKeepsReplyVersion() throws Exception {
    server.enqueue(
        new MockResponse()
            .setHeader("Content-Type", "text/event-stream")
            .setBody(
                "data: {\"choices\":[{\"delta\":{\"content\":\"原回复\"}}]}\n\ndata: [DONE]\n\n"));
    server.enqueue(
        new MockResponse()
            .setHeader("Content-Type", "text/event-stream")
            .setBody(
                "data: {\"choices\":[{\"delta\":{\"content\":\"新回复\"}}]}\n\ndata: [DONE]\n\n"));
    scenario.onActivity(
        a -> {
          Json.put(a.store.current(), "effort", "high");
          a.input.setText("测试重试");
          a.send();
        });
    waitFor(a -> a.generating == null && Json.array(a.store.current(), "messages").length() == 2);
    server.takeRequest();
    scenario.onActivity(
        a -> {
          Json.put(a.store.current(), "model", "different-model");
          Json.put(a.store.current(), "effort", "low");
          a.retry(1);
        });
    waitFor(a -> a.generating == null);
    JSONObject request =
        new JSONObject(server.takeRequest(2, TimeUnit.SECONDS).getBody().readUtf8());
    assertEquals("synthetic-model", request.getString("model"));
    assertEquals("high", request.getString("reasoning_effort"));
    scenario.onActivity(
        a -> {
          JSONObject m = Json.array(a.store.current(), "messages").optJSONObject(1);
          assertEquals("新回复", m.optString("text"));
          assertEquals("原回复", Json.array(m, "versions").optJSONObject(0).optString("text"));
          assertEquals("different-model", a.store.current().optString("model"));
          assertEquals("low", a.store.current().optString("effort"));
        });
  }

  @Test
  public void missingDoneNeverMarksPartialComplete() throws Exception {
    server.enqueue(
        new MockResponse()
            .setHeader("Content-Type", "text/event-stream")
            .setBody("data: {\"choices\":[{\"delta\":{\"content\":\"部分内容\"}}]}\n\n"));
    scenario.onActivity(
        a -> {
          a.input.setText("测试断线");
          a.send();
        });
    waitFor(a -> a.generating == null && Json.array(a.store.current(), "messages").length() == 2);
    scenario.onActivity(
        a -> {
          JSONObject m = Json.array(a.store.current(), "messages").optJSONObject(1);
          assertEquals("interrupted", m.optString("state"));
          assertEquals("部分内容", m.optString("text"));
          assertFalse(m.optString("issue").isEmpty());
        });
  }
}
