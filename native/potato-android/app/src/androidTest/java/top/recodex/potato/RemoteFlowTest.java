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
public class RemoteFlowTest {
  ActivityScenario<MainActivity> scenario;
  MockWebServer server;
  JSONObject device;

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
    HandshakeCertificates
        tlsServer = new HandshakeCertificates.Builder().heldCertificate(certificate).build(),
        tlsClient =
            new HandshakeCertificates.Builder()
                .addTrustedCertificate(certificate.certificate())
                .build();
    server = new MockWebServer();
    server.useHttps(tlsServer.sslSocketFactory(), false);
    server.start();
    OkHttpClient client =
        new Api()
            .client
            .newBuilder()
            .sslSocketFactory(tlsClient.sslSocketFactory(), tlsClient.trustManager())
            .readTimeout(2, TimeUnit.SECONDS)
            .build();
    device =
        Json.obj(
            "id",
            "00000000-0000-4000-8000-000000000001",
            "name",
            "合成测试电脑",
            "relay",
            server.url("/").toString().replaceAll("/$", ""));
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
          a.remote.visible = true;
          a.remote.device = device;
          a.remote.task(null);
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
    fail("Timed out awaiting remote state");
  }

  MockResponse reply(JSONObject value) {
    return new MockResponse()
        .setHeader("Content-Type", "application/json")
        .setBody(Json.obj("result", value).toString());
  }

  JSONObject chat() {
    return Json.obj("id", "test-task", "name", "测试任务", "status", "completed");
  }

  JSONObject snapshot() {
    return Json.obj(
        "chat",
        chat(),
        "status",
        "completed",
        "messages",
        new JSONArray(),
        "live",
        new JSONArray(),
        "approvals",
        new JSONArray(),
        "questions",
        new JSONArray());
  }

  @Test
  public void sendReceiptPreservesFollowupTypedInFlight() throws Exception {
    server.enqueue(reply(Json.obj("chat", chat())).setBodyDelay(500, TimeUnit.MILLISECONDS));
    server.enqueue(reply(snapshot()));
    scenario.onActivity(
        a -> {
          a.remote.input.setText("原指令");
          a.remote.send();
          a.remote.input.setText("后来输入的草稿");
        });
    RecordedRequest request = server.takeRequest(2, TimeUnit.SECONDS);
    assertNotNull(request);
    JSONObject wire = new JSONObject(request.getBody().readUtf8());
    assertEquals("send", wire.getString("op"));
    assertEquals("原指令", wire.getJSONObject("args").getString("text"));
    waitFor(a -> a.remote.chat != null);
    scenario.onActivity(
        a -> {
          assertEquals("后来输入的草稿", a.remote.input.getText().toString());
          assertFalse(a.remote.draft().has("pending"));
        });
  }

  @Test
  public void lostReplyRetainsSameOperationIdAndTarget() throws Exception {
    server.enqueue(new MockResponse().setSocketPolicy(SocketPolicy.DISCONNECT_AFTER_REQUEST));
    scenario.onActivity(
        a -> {
          a.remote.input.setText("仅执行一次");
          a.remote.send();
        });
    JSONObject first = new JSONObject(server.takeRequest(2, TimeUnit.SECONDS).getBody().readUtf8());
    waitFor(a -> !a.remote.busy);
    server.enqueue(reply(Json.obj("chat", chat(), "delivery", "recovered")));
    server.enqueue(reply(snapshot()));
    scenario.onActivity(
        a -> {
          JSONObject saved = a.remote.draft(), pending = saved.optJSONObject("pending");
          assertNotNull(pending);
          assertEquals(first.optString("id"), pending.optString("id"));
          assertEquals(a.remote.targetKey(device, ""), pending.optString("target"));
          a.remote.transmit(saved, pending);
        });
    JSONObject second =
        new JSONObject(server.takeRequest(2, TimeUnit.SECONDS).getBody().readUtf8());
    assertEquals(first.getString("id"), second.getString("id"));
    assertEquals(first.getJSONObject("args").toString(), second.getJSONObject("args").toString());
    waitFor(a -> a.remote.chat != null);
  }
}
