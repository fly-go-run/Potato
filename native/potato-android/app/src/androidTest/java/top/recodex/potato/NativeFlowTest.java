package top.recodex.potato;

import static androidx.test.espresso.Espresso.*;
import static androidx.test.espresso.action.ViewActions.*;
import static androidx.test.espresso.assertion.ViewAssertions.*;
import static androidx.test.espresso.matcher.ViewMatchers.*;
import static org.junit.Assert.*;

import android.content.Context;
import android.graphics.*;
import androidx.test.core.app.ActivityScenario;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import java.io.*;
import java.nio.charset.StandardCharsets;
import org.json.*;
import org.junit.*;
import org.junit.runner.RunWith;

@RunWith(AndroidJUnit4.class)
public class NativeFlowTest {
  ActivityScenario<MainActivity> scenario;

  @Before
  public void before() {
    Context c = InstrumentationRegistry.getInstrumentation().getTargetContext();
    for (String f : new String[] {"workspace.json", "workspace.json.bak", "workspace.json.new"})
      new File(c.getFilesDir(), f).delete();
    scenario = ActivityScenario.launch(MainActivity.class);
    scenario.onActivity(
        a -> {
          Json.put(a.store.root, "chats", new JSONArray());
          a.store.newChat();
          a.showChat();
        });
  }

  @After
  public void after() {
    scenario.close();
  }

  @Test
  public void homeAndNativeComposerAreVisible() {
    onView(withText("今天，想聊什么？")).check(matches(isDisplayed()));
    onView(withContentDescription("发送消息")).check(matches(isDisplayed()));
    onView(withContentDescription("打开侧栏")).perform(click());
    onView(withText("资料库")).check(matches(isDisplayed()));
  }

  @Test
  public void demoSendAndRelaunchPersist() throws Exception {
    onView(withContentDescription("问问 Potato")).perform(replaceText("测试本地持久化"));
    onView(withContentDescription("发送消息")).perform(click());
    Thread.sleep(800);
    scenario.onActivity(
        a -> {
          assertEquals(2, Json.array(a.store.current(), "messages").length());
          assertTrue(
              Json.array(a.store.current(), "messages")
                  .optJSONObject(1)
                  .optString("text")
                  .contains("没有调用模型"));
        });
    scenario.recreate();
    scenario.onActivity(
        a -> {
          assertEquals(
              "测试本地持久化",
              Json.array(a.store.current(), "messages").optJSONObject(0).optString("text"));
        });
  }

  @Test
  public void longDraftSurvivesRecreateAndNewChat() {
    String value = "长文草稿，键盘正常输入。".repeat(100);
    scenario.onActivity(
        a -> {
          a.input.setText(value);
          a.persist();
        });
    scenario.recreate();
    scenario.onActivity(
        a -> {
          assertEquals(value, a.input.getText().toString());
          String id = a.store.current().optString("id");
          a.store.newChat();
          a.showChat();
          assertEquals("", a.input.getText().toString());
          Json.put(a.store.root, "selected", id);
          a.showChat();
          assertEquals(value, a.input.getText().toString());
          a.input.setText("短");
          assertEquals("短", a.store.current().optString("input"));
        });
  }

  @Test
  public void keystoreCiphertextAndOriginBinding() {
    scenario.onActivity(
        a -> {
          try {
            a.store.secret("synthetic", "private-test-value");
            assertEquals("private-test-value", a.store.secret("synthetic"));
            String raw = a.getSharedPreferences("credentials", 0).getString("synthetic", "");
            assertFalse(raw.contains("private-test-value"));
            Json.put(a.store.settings(), "cloud", true);
            Json.put(
                a.store.settings(), "endpoint", "https://unrelated.example/v1/chat/completions");
            assertThrows(IOException.class, () -> a.store.token());
            a.store.secret("synthetic", "");
            assertEquals("", a.store.secret("synthetic"));
          } catch (Exception e) {
            throw new AssertionError(e);
          }
        });
  }

  @Test
  public void corruptStorageNeverOverwritesOriginal() {
    scenario.onActivity(
        a -> {
          try {
            File f = new File(a.getFilesDir(), "workspace.json");
            try (FileOutputStream out = new FileOutputStream(f)) {
              out.write("broken-json".getBytes(StandardCharsets.UTF_8));
            }
            Store bad = new Store(a);
            assertTrue(bad.corrupt);
            assertThrows(IOException.class, bad::save);
            assertEquals(
                "broken-json",
                new String(java.nio.file.Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8));
            a.persist();
          } catch (Exception e) {
            throw new AssertionError(e);
          }
        });
  }

  @Test
  public void interruptedReplyRestoresAsInterrupted() {
    scenario.onActivity(
        a -> {
          Json.array(a.store.current(), "messages")
              .put(
                  Json.obj(
                      "id",
                      "synthetic",
                      "role",
                      "assistant",
                      "text",
                      "已生成部分",
                      "state",
                      "streaming"));
          a.persist();
          Store restored = new Store(a);
          JSONObject m = Json.array(restored.current(), "messages").optJSONObject(0);
          assertEquals("interrupted", m.optString("state"));
          assertEquals("已生成部分", m.optString("text"));
        });
  }

  @Test
  public void branchKeepsOriginalMessagesAndDocuments() {
    scenario.onActivity(
        a -> {
          JSONObject original = a.store.current();
          JSONArray messages = Json.array(original, "messages");
          messages.put(Json.obj("role", "user", "text", "问题一"));
          messages.put(Json.obj("role", "assistant", "text", "回答一"));
          messages.put(Json.obj("role", "user", "text", "后续问题"));
          a.saveDocument(original, "- [ ] 待办");
          JSONObject branch = a.branch(original, 1);
          assertEquals(3, messages.length());
          assertEquals(1, Json.array(branch, "messages").length());
          assertEquals("- [ ] 待办", branch.optString("document"));
          a.saveDocument(branch, "- [x] 待办");
          assertEquals("- [ ] 待办", original.optString("document"));
          assertEquals(1, Json.array(branch, "documentVersions").length());
        });
  }

  @Test
  public void markdownTablesAndReasoningRender() {
    scenario.onActivity(
        a -> {
          JSONObject m =
              Json.obj(
                  "role",
                  "assistant",
                  "state",
                  "complete",
                  "text",
                  "# 标题\n\n"
                      + "**加粗**\n\n"
                      + "| A | B |\n"
                      + "|---|---|\n"
                      + "| 1 | 2 |\n\n"
                      + "```python\n"
                      + "print('中文')\n"
                      + "```",
                  "reasoning",
                  "公开思考过程",
                  "showReasoning",
                  true);
          Json.array(a.store.current(), "messages").put(m);
          a.renderMessages();
          assertTrue(a.rows.getChildCount() > 0);
        });
  }

  @Test
  public void modelCatalogDeduplicatesAndMigrates() {
    scenario.onActivity(
        a -> {
          try {
            JSONObject settings = Json.obj("model", "retired");
            SettingsScreen.installCatalog(
                settings,
                Json.obj(
                    "default_model",
                    "new",
                    "data",
                    new JSONArray()
                        .put(Json.obj("id", "new", "name", "新模型"))
                        .put(Json.obj("id", "new"))
                        .put(Json.obj("id", ""))));
            assertEquals("new", settings.optString("model"));
            assertEquals(1, Json.array(settings, "models").length());
          } catch (Exception e) {
            throw new AssertionError(e);
          }
        });
  }

  @Test
  public void remoteDraftsAreBoundToComputerAndTask() {
    scenario.onActivity(
        a -> {
          RemoteScreen r = a.remote;
          JSONObject one = Json.obj("id", "one", "relay", Store.RELAY),
              two = Json.obj("id", "two", "relay", Store.RELAY);
          assertNotEquals(r.targetKey(one, "task"), r.targetKey(two, "task"));
          assertNotEquals(r.targetKey(one, "task"), r.targetKey(one, "other"));
        });
  }

  @Test
  public void nativeTextAndImageAttachmentsProduceActualWirePayload() {
    scenario.onActivity(
        a -> {
          try {
            File dir = new File(a.getFilesDir(), "attachments");
            dir.mkdirs();
            File f = new File(dir, "synthetic.txt");
            try (FileOutputStream out = new FileOutputStream(f)) {
              out.write("文件内容：中文".getBytes(StandardCharsets.UTF_8));
            }
            JSONObject text = a.files.importUri(a.files.uri(f));
            assertEquals("文件内容：中文", text.optString("text"));
            File image = new File(dir, "synthetic.jpg");
            Bitmap b = Bitmap.createBitmap(24, 24, Bitmap.Config.ARGB_8888);
            b.eraseColor(Color.GREEN);
            try (FileOutputStream out = new FileOutputStream(image)) {
              b.compress(Bitmap.CompressFormat.JPEG, 90, out);
            }
            b.recycle();
            JSONObject photo = a.files.importUri(a.files.uri(image));
            JSONArray wire =
                a.files.wire(
                    new JSONArray()
                        .put(
                            Json.obj(
                                "role",
                                "user",
                                "text",
                                "测试附件",
                                "attachments",
                                new JSONArray().put(text).put(photo))));
            JSONArray parts = wire.getJSONObject(0).getJSONArray("content");
            assertTrue(parts.getJSONObject(0).getString("text").contains("文件内容：中文"));
            assertTrue(
                parts
                    .getJSONObject(1)
                    .getJSONObject("image_url")
                    .getString("url")
                    .startsWith("data:image/jpeg;base64,"));
          } catch (Exception e) {
            throw new AssertionError(e);
          }
        });
  }
}
