package top.recodex.potato;

import android.app.AlertDialog;
import android.graphics.*;
import android.text.InputType;
import android.view.*;
import android.widget.*;
import java.io.IOException;
import okhttp3.Call;
import org.json.*;

final class SettingsForm {
  final MainActivity a;
  final SettingsScreen owner;
  JSONObject initial;
  AlertDialog dialog;
  Call test;
  EditText endpoint, model, token, prompt;
  Switch demo, haptics;
  TextView testStatus;
  Button testButton;

  SettingsForm(MainActivity a, SettingsScreen owner) {
    this.a = a;
    this.owner = owner;
  }

  void open() {
    a.interruptVoice();
    initial = Json.copy(a.store.settings());
    LinearLayout content = a.column();
    content.setPadding(a.dp(16), a.dp(26), a.dp(16), a.dp(24));
    LinearLayout group = ParitySheet.group(a, content), brand = a.row();
    brand.setPadding(a.dp(16), a.dp(24), a.dp(16), a.dp(24));
    ImageView mark = new ImageView(a);
    mark.setImageResource(R.drawable.potato_mark);
    mark.setBackground(a.bg(MainActivity.CANVAS, 14));
    mark.setClipToOutline(true);
    brand.addView(mark, new LinearLayout.LayoutParams(a.dp(56), a.dp(56)));
    LinearLayout labels = a.column();
    labels.setPadding(a.dp(14), 0, 0, 0);
    TextView title = a.text("Potato", 24, MainActivity.INK);
    title.setTypeface(null, Typeface.BOLD);
    labels.addView(title);
    labels.addView(a.text("你的想法，随时继续。", 15, MainActivity.MUTED));
    brand.addView(labels);
    group.addView(brand);
    section(content, "云端模型");
    group = ParitySheet.group(a, content);
    owner.loginStatus = a.text(initial.optString("email"), 14, MainActivity.MUTED);
    if (initial.has("email")) group.addView(owner.loginStatus);
    owner.loginButton =
        a.button(
            a.store.root.has("login")
                ? "继续登录"
                : initial.optBoolean("cloud") ? "管理云端账号" : "登录 Cloudflare，使用云端模型",
            () -> owner.login("cloud"));
    owner.loginButton.setGravity(Gravity.START | Gravity.CENTER_VERTICAL);
    group.addView(owner.loginButton, new LinearLayout.LayoutParams(-1, a.dp(54)));
    if (initial.optBoolean("cloud")) {
      group.addView(
          a.button(
              "刷新云端模型", () -> owner.refreshModels(a.store.settings(), () -> a.toast("模型列表已更新"))));
      group.addView(a.button("退出云端账号", owner::logout));
    }
    group = ParitySheet.group(a, content);
    demo = toggle(group, "本地体验模式", initial.optBoolean("demo"));
    demo.setContentDescription("本地体验（不调用模型）");
    note(content, "发送时，会将当前对话及其附件交给下方配置的服务。");
    endpoint = a.field("完整 HTTPS /v1/chat/completions 地址", initial.optString("endpoint"));
    endpoint.setHint("完整接口地址（HTTPS）");
    endpoint.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_URI);
    model = a.field("模型名称", initial.optString("model"));
    token = a.field("连接令牌（保存在本机）", "");
    token.setHint("连接令牌（可选）");
    token.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
    try {
      if (!initial.optBoolean("cloud")) token.setText(a.store.token());
    } catch (Exception e) {
      a.error(e);
    }
    if (!initial.optBoolean("cloud")) {
      section(content, "模型连接");
      group = ParitySheet.group(a, content);
      for (EditText field : new EditText[] {endpoint, model, token}) {
        field.setBackgroundColor(Color.TRANSPARENT);
        field.setTextSize(17);
        group.addView(field);
      }
      note(
          group,
          "支持 Cloudflare Worker 提供的 OpenAI 兼容流式接口，例如 /v1/chat/completions。令牌只保存在此设备的 Android"
              + " Keystore。");
    }
    testStatus = a.text("", 13, MainActivity.MUTED);
    if (!initial.optBoolean("cloud")) {
      group = ParitySheet.group(a, content);
      testButton = a.button("测试连接", this::test);
      group.addView(testButton);
      group.addView(testStatus);
      note(content, "测试会发送一条简短请求。连接成功后，仍需点右上角保存。");
    }
    String original = endpoint.getText().toString();
    a.watch(
        endpoint,
        s -> {
          if (!s.equals(original)) token.setText("");
          invalidateTest();
        });
    a.watch(model, s -> invalidateTest());
    a.watch(token, s -> invalidateTest());
    section(content, "体验");
    group = ParitySheet.group(a, content);
    haptics = toggle(group, "触感反馈", initial.optBoolean("haptics", true));
    section(content, "回复偏好");
    group = ParitySheet.group(a, content);
    prompt = a.field("回复偏好", initial.optString("prompt"));
    prompt.setMinLines(3);
    prompt.setBackgroundColor(Color.TRANSPARENT);
    group.addView(prompt);
    section(content, "数据");
    group = ParitySheet.group(a, content);
    ParitySheet.item(
        a, group, "记忆与历史", null, "chevron-right", false, () -> new RecallScreen(a).open());
    group.addView(a.text("对话保存在本机 · " + a.store.chats().length() + " 段对话", 14, MainActivity.MUTED));
    note(content, "Potato Android · 与 iOS 共用模型、语音和远程服务。");
    dialog = a.sheet("设置", content);
    owner.dialog = dialog;
    ParitySheet.actions(a, dialog, "取消", dialog::dismiss, "保存", this::save);
    dialog.setOnDismissListener(
        d -> {
          if (test != null) test.cancel();
          test = null;
        });
  }

  void section(LinearLayout l, String title) {
    TextView t = a.text(title, 15, MainActivity.MUTED);
    t.setTypeface(null, Typeface.BOLD);
    t.setPadding(a.dp(16), a.dp(22), a.dp(16), a.dp(10));
    l.addView(t);
  }

  void note(LinearLayout l, String value) {
    TextView t = a.text(value, 13, MainActivity.MUTED);
    t.setPadding(a.dp(16), a.dp(8), a.dp(16), a.dp(12));
    l.addView(t);
  }

  Switch toggle(LinearLayout l, String title, boolean on) {
    Switch s = new Switch(a);
    s.setText(title);
    s.setTextSize(17);
    s.setTextColor(MainActivity.INK);
    s.setPadding(a.dp(16), a.dp(14), a.dp(16), a.dp(14));
    s.setChecked(on);
    l.addView(s);
    return s;
  }

  void invalidateTest() {
    if (test != null) test.cancel();
    test = null;
    testStatus.setText("");
    if (testButton != null) testButton.setText("测试连接");
  }

  void test() {
    if (test != null) {
      test.cancel();
      test = null;
      testStatus.setText("测试已取消");
      testButton.setText("测试连接");
      return;
    }
    try {
      String url = endpoint.getText().toString().trim(),
          key = token.getText().toString(),
          id = model.getText().toString().trim();
      if (id.isEmpty()) throw new IOException("请填写模型名称。");
      Call call =
          a.api.streamCall(
              url,
              key,
              Json.obj(
                  "model",
                  id,
                  "stream",
                  true,
                  "max_tokens",
                  16,
                  "messages",
                  new JSONArray().put(Json.obj("role", "user", "content", "请只回复 OK"))));
      test = call;
      testButton.setText("取消测试");
      testStatus.setText("正在测试…");
      a.worker.execute(
          () -> {
            String issue = null;
            boolean[] visible = {false};
            try {
              a.api.stream(
                  call,
                  event -> {
                    JSONArray choices = event.optJSONArray("choices");
                    if (choices != null && choices.length() > 0) {
                      JSONObject delta = choices.optJSONObject(0).optJSONObject("delta");
                      if (delta != null && !delta.optString("content").trim().isEmpty())
                        visible[0] = true;
                    }
                  });
              if (!visible[0]) throw new IOException("服务未返回文字。");
            } catch (Exception e) {
              issue = e.getMessage();
            }
            String failure = issue;
            a.main.post(
                () -> {
                  if (test != call) return;
                  test = null;
                  testButton.setText("测试连接");
                  testStatus.setText(failure == null ? "连接成功，保存后生效。" : failure);
                });
          });
    } catch (Exception e) {
      testStatus.setText(e.getMessage());
    }
  }

  void save() {
    try {
      String url = endpoint.getText().toString().trim(), id = model.getText().toString().trim();
      if (!demo.isChecked()) {
        Api.url(url);
        if (id.isEmpty()) throw new IOException("请填写模型名称或登录云端。");
      }
      JSONObject current = a.store.settings();
      boolean cloud =
          initial.optBoolean("cloud") && url.equals(Store.RELAY + "/v1/chat/completions");
      if (!cloud) a.store.secret("custom:" + url, token.getText().toString());
      boolean changed = !url.equals(current.optString("endpoint"));
      Json.put(current, "demo", demo.isChecked());
      Json.put(current, "endpoint", url);
      Json.put(current, "model", id);
      Json.put(current, "cloud", cloud);
      Json.put(current, "prompt", prompt.getText().toString());
      Json.put(current, "haptics", haptics.isChecked());
      if (changed) {
        current.remove("models");
        for (int i = 0; i < a.store.chats().length(); i++) {
          JSONObject c =
              a.store
                  .chats()
                  .optJSONObject(
                      i); /* Keep the previous service identity until the user explicitly reselects a model. */
          if (!c.has("endpoint")) Json.put(c, "endpoint", initial.optString("endpoint"));
        }
      }
      a.persist();
      dialog.dismiss();
      a.showChat();
    } catch (Exception e) {
      a.message("无法保存", e.getMessage());
    }
  }
}
