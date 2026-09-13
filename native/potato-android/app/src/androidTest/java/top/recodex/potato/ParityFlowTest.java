package top.recodex.potato;

import static androidx.test.espresso.Espresso.*;
import static androidx.test.espresso.action.ViewActions.*;
import static androidx.test.espresso.assertion.ViewAssertions.*;
import static androidx.test.espresso.matcher.ViewMatchers.*;
import static org.junit.Assert.*;

import android.content.*;
import android.graphics.Bitmap;
import android.provider.MediaStore;
import androidx.test.core.app.ActivityScenario;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import java.io.*;
import org.json.*;
import org.junit.*;
import org.junit.runner.RunWith;

@RunWith(AndroidJUnit4.class)
public class ParityFlowTest {
  ActivityScenario<MainActivity> scenario;

  @Before
  public void setup() {
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
    scenario.onActivity(
        a -> {
          JSONObject s = a.store.settings();
          Json.put(s, "model", "quick");
          Json.put(
              s,
              "models",
              new JSONArray()
                  .put(Json.obj("id", "quick", "name", "快速模型"))
                  .put(
                      Json.obj(
                          "id",
                          "deep",
                          "name",
                          "深入模型",
                          "thinking_modes",
                          new JSONArray().put("enabled").put("disabled"),
                          "reasoning_effort_options",
                          new JSONArray().put("low").put("high"))));
          a.showChat();
        });
  }

  @After
  public void cleanup() {
    scenario.close();
  }

  static void capture(String name) throws Exception {
    InstrumentationRegistry.getInstrumentation().waitForIdleSync();
    Thread.sleep(350);
    Context c = InstrumentationRegistry.getInstrumentation().getTargetContext();
    ContentValues values = new ContentValues();
    values.put(MediaStore.Images.Media.DISPLAY_NAME, name + ".png");
    values.put(MediaStore.Images.Media.MIME_TYPE, "image/png");
    values.put(MediaStore.Images.Media.RELATIVE_PATH, "Pictures/PotatoParity");
    android.net.Uri uri =
        c.getContentResolver().insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values);
    try (OutputStream out = c.getContentResolver().openOutputStream(uri)) {
      Bitmap image =
          InstrumentationRegistry.getInstrumentation().getUiAutomation().takeScreenshot();
      assertNotNull(image);
      image.compress(Bitmap.CompressFormat.PNG, 100, out);
      image.recycle();
    }
  }

  @Test
  public void homeModelThinkingPreservesDraftAndSameModelChoice() throws Exception {
    onView(withText("今天，想聊什么？")).check(matches(isDisplayed()));
    scenario.onActivity(a -> assertFalse(a.send.isEnabled()));
    capture("01-home");
    scenario.onActivity(
        a -> {
          a.input.setText("keep draft");
          a.settingsScreen.models();
        });
    capture("02-models");
    onView(withText("深入模型")).perform(click());
    onView(withText("思考")).perform(click());
    onView(withText("高")).perform(click());
    capture("03-thinking");
    onView(withText("返回模型")).perform(click());
    onView(withText("深入模型")).perform(click());
    onView(withContentDescription("完成")).perform(click());
    scenario.onActivity(
        a -> {
          assertEquals("high", a.store.current().optString("effort"));
          assertEquals("enabled", a.store.current().optString("thinking"));
          assertEquals("keep draft", a.input.getText().toString());
        });
    scenario.recreate();
    scenario.onActivity(a -> assertEquals("high", a.store.current().optString("effort")));
  }

  @Test
  public void replyMenuVersionAndCancelModelHaveSameSemantics() throws Exception {
    scenario.onActivity(
        a -> {
          JSONArray ms = Json.array(a.store.current(), "messages");
          ms.put(
              Json.obj(
                  "id", Json.id(), "role", "user", "text", "check model", "state", "complete"));
          ms.put(
              Json.obj(
                  "id",
                  Json.id(),
                  "role",
                  "assistant",
                  "text",
                  "MODEL=quick;THINKING=default;EFFORT=default",
                  "state",
                  "complete",
                  "model",
                  "quick"));
          a.showChat();
        });
    capture("04-reply");
    onView(withContentDescription("回复操作")).perform(click());
    capture("05-reply-menu");
    onView(withText("换模型重新回答")).perform(click());
    onView(withText("深入模型")).perform(click());
    onView(withText("思考")).perform(click());
    onView(withText("高")).perform(click());
    onView(withContentDescription("完成")).perform(click());
    scenario.onActivity(
        a -> {
          assertEquals(
              "quick", a.store.current().optString("model", a.store.settings().optString("model")));
          JSONObject m = Json.array(a.store.current(), "messages").optJSONObject(1);
          Json.put(
              m,
              "versions",
              new JSONArray()
                  .put(
                      Json.obj(
                          "text",
                          "旧回复",
                          "role",
                          "assistant",
                          "state",
                          "complete",
                          "model",
                          "deep")));
          a.renderMessages();
        });
    onView(withContentDescription("上一版回复")).perform(click());
    scenario.onActivity(
        a ->
            assertEquals(
                "旧回复",
                Json.array(a.store.current(), "messages").optJSONObject(1).optString("text")));
    onView(withContentDescription("下一版回复")).perform(click());
    scenario.onActivity(
        a ->
            assertTrue(
                Json.array(a.store.current(), "messages")
                    .optJSONObject(1)
                    .optString("text")
                    .startsWith("MODEL=quick")));
  }

  @Test
  public void documentEditingCancelAndHistoryAreTransactional() throws Exception {
    String original =
        "# 给周末留一点空白\n\n"
            + "只安排两件想做的事，给临时起意留点余地。\n\n"
            + "## 周六 · 出门走走\n\n"
            + "- [ ] 去附近的公园走走，晒晒太阳\n"
            + "- [ ] 找一家喜欢的咖啡店，带上一本书\n\n"
            + "## 周日 · 慢慢收尾\n\n"
            + "- [ ] 整理一下房间，让下周更轻松\n"
            + "- [ ] 做一顿喜欢的饭，早点休息";
    scenario.onActivity(
        a -> {
          JSONObject c = a.store.current();
          Json.array(c, "messages")
              .put(
                  Json.obj(
                      "id",
                      Json.id(),
                      "role",
                      "user",
                      "text",
                      "把这些想法整理成一个周末计划",
                      "state",
                      "complete"))
              .put(
                  Json.obj(
                      "id",
                      Json.id(),
                      "role",
                      "assistant",
                      "text",
                      "整理成了一份可以继续修改的计划。",
                      "state",
                      "complete"));
          a.saveDocument(c, original);
          a.showChat();
          a.document(c);
        });
    capture("06-document");
    onView(withContentDescription("编辑文稿")).perform(click());
    capture("07-document-edit");
    onView(withContentDescription("编辑文稿")).perform(replaceText("不应保存的修改"));
    onView(withText("取消")).perform(click());
    scenario.onActivity(a -> assertEquals(original, a.store.current().optString("document")));
    onView(withText("去附近的公园走走，晒晒太阳")).perform(click());
    scenario.onActivity(
        a -> {
          assertTrue(a.store.current().optString("document").contains("[x]"));
          assertEquals(1, Json.array(a.store.current(), "documentVersions").length());
          assertTrue(a.input.isShown());
        });
  }

  @Test
  public void settingsCancelDoesNotChangeConnectionOrPreferences() throws Exception {
    scenario.onActivity(
        a -> {
          Json.put(a.store.settings(), "endpoint", "");
          Json.put(a.store.settings(), "model", "");
          a.settingsScreen.open();
        });
    capture("08-settings");
    onView(withContentDescription("完整 HTTPS /v1/chat/completions 地址"))
        .perform(replaceText("https://example.invalid/v1/chat/completions"));
    onView(withText("取消")).perform(click());
    scenario.onActivity(a -> assertEquals("", a.store.settings().optString("endpoint")));
  }

  @Test
  public void sidebarAndMemoryScreensAreReachable() throws Exception {
    scenario.onActivity(
        a -> {
          for (int i = 0; i < 12; i++) {
            JSONObject c = a.store.newChat();
            Json.put(c, "title", "合成对话 " + i);
          }
          a.showChat();
          a.sidebar();
        });
    capture("09-sidebar");
    androidx.test.espresso.Espresso.pressBack();
    scenario.onActivity(a -> new RecallScreen(a).open());
    capture("10-recall");
    onView(withText("跨对话检索")).check(matches(isDisplayed()));
  }

  @Test
  public void allShippedIconsLoadAndRemoteEmptyMatchesReference() throws Exception {
    scenario.onActivity(
        a -> {
          for (java.lang.reflect.Field field : R.drawable.class.getFields())
            if (field.getName().startsWith("lucide_")) {
              try {
                assertNotNull(a.getDrawable(field.getInt(null)));
              } catch (Exception e) {
                throw new AssertionError(field.getName(), e);
              }
            }
          a.remote.open();
        });
    capture("11-remote-empty");
    onView(withText("让电脑上的任务，随你继续")).check(matches(isDisplayed()));
  }

  @Test
  public void firstRunSampleIsEditableAndExcludedFromRecall() throws Exception {
    scenario.onActivity(
        a -> {
          JSONObject c = ExampleDocument.create(a.store);
          assertTrue(c.optBoolean("example"));
          assertEquals(2, Json.array(c, "messages").length());
          a.showChat();
        });
    capture("12-first-run-document");
    onView(withText("周末计划 · 已保存在本机")).check(matches(isDisplayed()));
    onView(withText("去附近的公园走走，晒晒太阳")).perform(click());
    scenario.onActivity(
        a -> {
          assertTrue(a.store.current().optString("document").contains("[x]"));
          assertTrue(a.store.current().optBoolean("sampleDocument"));
        });
  }

  @Test
  public void largeTextLongDraftKeepsComposerControlsReachable() throws Exception {
    scenario.onActivity(
        a -> {
          android.content.res.Configuration config =
              new android.content.res.Configuration(a.getResources().getConfiguration());
          config.fontScale = 1.5f;
          a.getResources().updateConfiguration(config, a.getResources().getDisplayMetrics());
          a.showChat();
          a.input.setText("这是一段需要展开编辑的长文草稿。".repeat(80));
        });
    capture("14-large-text-long-draft");
    onView(withContentDescription("发送消息")).check(matches(isDisplayed()));
    onView(withContentDescription("语音输入")).check(matches(isDisplayed()));
    onView(withText("展开")).perform(click());
    capture("15-expanded-draft");
    onView(withText("收起")).perform(click());
    scenario.onActivity(
        a -> {
          assertTrue(a.input.getText().toString().length() > 800);
          android.content.res.Configuration config =
              new android.content.res.Configuration(a.getResources().getConfiguration());
          config.fontScale = 1;
          a.getResources().updateConfiguration(config, a.getResources().getDisplayMetrics());
        });
  }

  @Test
  public void photoStripRemovalGalleryAndSentGridWork() throws Exception {
    scenario.onActivity(
        a -> {
          JSONArray attachments = Json.array(a.store.current(), "attachments");
          try {
            File dir = new File(a.getFilesDir(), "attachments");
            dir.mkdirs();
            for (int i = 0; i < 4; i++) {
              String file = "parity-image-" + i + ".png";
              android.graphics.Bitmap bitmap =
                  android.graphics.Bitmap.createBitmap(
                      300, 300, android.graphics.Bitmap.Config.ARGB_8888);
              bitmap.eraseColor(new int[] {0xffe1b37f, 0xffacc5a3, 0xffb9c7dc, 0xffd9b9ca}[i]);
              try (FileOutputStream output = new FileOutputStream(new File(dir, file))) {
                bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, output);
              }
              bitmap.recycle();
              attachments.put(Json.obj("file", file, "name", "合成图片 " + i, "type", "image/png"));
            }
            a.renderAttachments();
          } catch (Exception e) {
            throw new AssertionError(e);
          }
        });
    capture("16-pending-images");
    onView(withContentDescription("预览 合成图片 0")).perform(click());
    capture("17-image-gallery");
    onView(withContentDescription("下一张")).perform(click());
    onView(withText("2 / 4")).check(matches(isDisplayed()));
    onView(withContentDescription("关闭预览")).perform(click());
    onView(withContentDescription("移除附件 合成图片 0")).perform(click());
    scenario.onActivity(
        a -> {
          assertEquals(3, Json.array(a.store.current(), "attachments").length());
          a.send();
        });
    Thread.sleep(700);
    capture("18-sent-image-grid");
    scenario.onActivity(
        a ->
            assertEquals(
                3,
                Json.array(
                        Json.array(a.store.current(), "messages").optJSONObject(0), "attachments")
                    .length()));
  }

  @Test
  public void recallVersionFollowsReplySelectionAndRestoresIdentity() {
    scenario.onActivity(
        a -> {
          JSONObject m =
              Json.obj(
                  "id",
                  Json.id(),
                  "role",
                  "assistant",
                  "text",
                  "new",
                  "state",
                  "complete",
                  "versions",
                  new JSONArray()
                      .put(
                          Json.obj(
                              "id",
                              Json.id(),
                              "role",
                              "assistant",
                              "text",
                              "old",
                              "state",
                              "complete")));
          String latest = RecallScreen.messageVersion(m);
          a.selectVersion(m, 0);
          assertNotEquals(latest, RecallScreen.messageVersion(m));
          a.selectVersion(m, 1);
          assertEquals(latest, RecallScreen.messageVersion(m));
        });
  }

  @Test
  public void removingPairedComputerClearsOnlyItsLocalCredential() throws Exception {
    scenario.onActivity(
        a -> {
          try {
            JSONObject one =
                Json.obj(
                    "id",
                    "test-one",
                    "name",
                    "配对电脑",
                    "relay",
                    "https://example.invalid",
                    "paired",
                    true);
            JSONObject two =
                Json.obj(
                    "id",
                    "test-two",
                    "name",
                    "保留电脑",
                    "relay",
                    "https://example.invalid",
                    "paired",
                    true);
            Json.put(a.store.root, "pairedDevices", new JSONArray().put(one).put(two));
            a.store.secret("device:" + a.remote.targetKey(one, ""), "synthetic-one");
            a.store.secret("device:" + a.remote.targetKey(two, ""), "synthetic-two");
            RemoteHome home = new RemoteHome(a.remote);
            home.devices.add(one);
            home.devices.add(two);
            home.selected = "test-one";
            home.forget(one);
            assertEquals("", a.store.secret("device:" + a.remote.targetKey(one, "")));
            assertEquals("synthetic-two", a.store.secret("device:" + a.remote.targetKey(two, "")));
            assertEquals(
                "test-two",
                Json.array(a.store.root, "pairedDevices").getJSONObject(0).getString("id"));
            assertEquals(1, home.devices.size());
            assertEquals("", home.selected);
          } catch (Exception e) {
            throw new AssertionError(e);
          }
        });
  }

  @Test
  public void pdfPreviewRendersPagesInsideApp() throws Exception {
    scenario.onActivity(
        a -> {
          try {
            File folder = new File(a.getFilesDir(), "attachments");
            folder.mkdirs();
            File file = new File(folder, "parity-preview.pdf");
            android.graphics.pdf.PdfDocument document = new android.graphics.pdf.PdfDocument();
            for (int i = 0; i < 2; i++) {
              android.graphics.pdf.PdfDocument.Page page =
                  document.startPage(
                      new android.graphics.pdf.PdfDocument.PageInfo.Builder(300, 400, i + 1)
                          .create());
              android.graphics.Paint paint = new android.graphics.Paint();
              paint.setTextSize(20);
              page.getCanvas().drawText("Synthetic page " + (i + 1), 30, 50, paint);
              document.finishPage(page);
            }
            try (FileOutputStream out = new FileOutputStream(file)) {
              document.writeTo(out);
            }
            document.close();
            a.openAttachment(
                Json.obj("file", file.getName(), "name", "合成 PDF", "type", "application/pdf"));
          } catch (Exception e) {
            throw new AssertionError(e);
          }
        });
    onView(withText("1 / 2"))
        .inRoot(androidx.test.espresso.matcher.RootMatchers.isDialog())
        .check(matches(isDisplayed()));
    onView(withContentDescription("下一页")).perform(click());
    onView(withText("2 / 2")).check(matches(isDisplayed()));
    capture("19-pdf-preview");
    onView(withContentDescription("关闭预览")).perform(click());
  }
}
