package top.recodex.potato;

import android.app.AlertDialog;
import android.widget.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.time.Instant;
import java.util.*;
import okhttp3.HttpUrl;
import org.json.*;

final class RecallScreen {
  final MainActivity a;
  AlertDialog dialog;
  boolean busy;

  RecallScreen(MainActivity a) {
    this.a = a;
  }

  static String messageVersion(JSONObject message) {
    if (!message.optString("role").equals("assistant")) return "";
    if (!message.optString("selectedVersion").isEmpty())
      return message.optString("selectedVersion");
    if (!message.optString("id").isEmpty()) return message.optString("id").toLowerCase(Locale.ROOT);
    return UUID.nameUUIDFromBytes(
            (message.optString("text")
                    + "\n"
                    + message.optString("model")
                    + "\n"
                    + message.optLong("createdAt"))
                .getBytes(StandardCharsets.UTF_8))
        .toString();
  }

  void open() {
    render();
    if (!a.store.settings().optBoolean("demo")) perform(null);
  }

  String connection(JSONObject settings, String token) throws Exception {
    return hash(settings.optString("endpoint") + token);
  }

  JSONArray visibleMemories() {
    try {
      if (connection(a.store.settings(), a.store.token())
          .equals(a.store.root.optString("memoriesConnection")))
        return Json.array(a.store.root, "memories");
    } catch (Exception ignored) {
    }
    return new JSONArray();
  }

  void perform(JSONObject mutation) {
    if (busy) return;
    JSONObject settings = Json.copy(a.store.settings());
    String token;
    try {
      token = a.store.token();
    } catch (Exception e) {
      a.error(e);
      return;
    }
    busy = true;
    a.worker.execute(
        () -> {
          JSONObject result = null;
          Exception failure = null;
          try {
            if (mutation != null) request(a, settings, token, "memory", mutation);
            result = request(a, settings, token, "status", null);
          } catch (Exception e) {
            failure = e;
          }
          JSONObject value = result;
          Exception issue = failure;
          a.main.post(
              () -> {
                busy = false;
                try {
                  if (value != null
                      && connection(settings, token)
                          .equals(connection(a.store.settings(), a.store.token()))) {
                    Json.put(a.store.root, "memories", Json.array(value, "memories"));
                    Json.put(a.store.root, "memoriesConnection", connection(settings, token));
                    a.persist();
                    if (dialog != null && dialog.isShowing()) render();
                  }
                } catch (Exception e) {
                  a.error(e);
                }
                if (issue != null) a.error(issue);
              });
        });
  }

  void render() {
    if (dialog != null) dialog.dismiss();
    LinearLayout content = a.column();
    content.setPadding(a.dp(20), a.dp(12), a.dp(20), a.dp(24));
    JSONObject settings = a.store.settings();
    LinearLayout group = ParitySheet.group(a, content);
    Switch enabled = toggle(group, "跨对话检索", settings.optBoolean("recallEnabled"));
    Switch automatic = toggle(group, "自动记录长期记忆", settings.optBoolean("automaticMemory"));
    automatic.setEnabled(enabled.isChecked());
    enabled.setOnCheckedChangeListener(
        (v, on) -> {
          Json.put(settings, "recallEnabled", on);
          a.persist();
          automatic.setEnabled(on);
          if (on) sync(false);
        });
    automatic.setOnCheckedChangeListener(
        (v, on) -> {
          Json.put(settings, "automaticMemory", on);
          a.persist();
        });
    note(
        content,
        "开启后，消息文字会同步到云端，并交给 E2B"
            + " 按需检索。附件、示例和排除的对话不参与。关闭后停止本机检索与新增同步，删除和排除仍会同步，已保存的其他云端历史保留。自动记忆保存你明确表达的长期偏好，也可以手动添加。");
    group = ParitySheet.group(a, content);
    Button sync = a.button(busy ? "正在同步…" : "立即同步历史", () -> sync(true));
    sync.setEnabled(!busy && settings.has("recallEnabled") && !settings.optBoolean("demo"));
    group.addView(sync);
    note(content, "发送消息前会确认历史同步完成；离线、同步冲突或服务未配置时会提示重试。关键词检索可能需要换词，不保证找全所有语义相近的内容。");
    content.addView(a.text("长期记忆", 13, MainActivity.MUTED));
    group = ParitySheet.group(a, content);
    JSONArray memories = visibleMemories();
    if (memories.length() == 0) group.addView(a.text("还没有保存的记忆", 16, MainActivity.MUTED));
    for (int i = 0; i < memories.length(); i++) {
      JSONObject m = memories.optJSONObject(i);
      group.addView(a.text(m.optString("text"), 17, MainActivity.INK));
      LinearLayout buttons = a.row();
      buttons.addView(a.button("编辑", () -> edit(m)));
      buttons.addView(a.button("忘记", () -> save(m, "", true)));
      group.addView(buttons);
    }
    Button add = a.button("添加记忆", () -> edit(null));
    add.setEnabled(!settings.optBoolean("demo"));
    group.addView(add);
    note(content, "忘记后，该条自动记忆的原始消息不会再次用于自动记忆；原始对话仍可被历史检索，需彻底排除时请关闭下方对应对话。");
    content.addView(a.text("参与检索的对话", 13, MainActivity.MUTED));
    group = ParitySheet.group(a, content);
    for (int i = a.store.chats().length() - 1; i >= 0; i--) {
      JSONObject c = a.store.chats().optJSONObject(i);
      if (c.optBoolean("deleted")
          || c.optBoolean("example")
          || Json.array(c, "messages").length() == 0) continue;
      Switch included = toggle(group, c.optString("title"), !c.optBoolean("recallExcluded"));
      included.setOnCheckedChangeListener(
          (v, on) -> {
            Json.put(c, "recallExcluded", !on);
            if (!on) invalidate(c);
            a.persist();
            sync(true);
          });
    }
    note(content, "关闭某段对话后，会同步移除其云端内容和来源记忆。删除对话也会移除；网络不可用时需联网重试同步。");
    dialog = a.sheet("记忆与历史", content);
  }

  Switch toggle(LinearLayout parent, String title, boolean checked) {
    Switch s = new Switch(a);
    s.setText(title);
    s.setTextSize(17);
    s.setTextColor(MainActivity.INK);
    s.setPadding(a.dp(16), a.dp(16), a.dp(16), a.dp(16));
    s.setChecked(checked);
    parent.addView(s);
    return s;
  }

  void note(LinearLayout l, String value) {
    TextView t = a.text(value, 13, MainActivity.MUTED);
    t.setPadding(a.dp(8), 0, a.dp(8), a.dp(20));
    l.addView(t);
  }

  void edit(JSONObject m) {
    EditText text = a.field("记忆内容", m == null ? "" : m.optString("text"));
    text.setMinLines(8);
    LinearLayout l = a.column();
    l.addView(text);
    AlertDialog editor = a.sheet(m == null ? "添加记忆" : "编辑记忆", l);
    Button save =
        a.button(
            "保存",
            () -> {
              editor.dismiss();
              save(m, text.getText().toString().trim(), false);
            });
    l.addView(save);
    save.setEnabled(text.length() > 0 && text.length() <= 1500);
    a.watch(text, s -> save.setEnabled(!s.trim().isEmpty() && s.length() <= 1500));
    l.addView(a.button("取消", editor::dismiss));
  }

  void save(JSONObject memory, String text, boolean forget) {
    perform(
        Json.obj(
            "id",
            memory == null ? Json.id() : memory.optString("id"),
            "text",
            text,
            "base",
            memory == null ? JSONObject.NULL : memory.opt("revision"),
            "forget",
            forget));
  }

  void sync(boolean cleanup) {
    if (busy || a.store.settings().optBoolean("demo") || !a.store.settings().has("recallEnabled"))
      return;
    busy = true;
    JSONObject settings = Json.copy(a.store.settings());
    JSONArray chats = Json.copy(a.store.chats());
    String credential;
    try {
      credential = a.store.token();
    } catch (Exception e) {
      busy = false;
      a.error(e);
      return;
    }
    a.worker.execute(
        () -> {
          JSONObject result = null;
          Exception failure = null;
          try {
            result =
                synchronize(a, settings, credential, chats, !settings.optBoolean("recallEnabled"));
          } catch (Exception e) {
            failure = e;
          }
          JSONObject value = result;
          Exception issue = failure;
          a.main.post(
              () -> {
                busy = false;
                try {
                  if (value != null
                      && connection(settings, credential)
                          .equals(connection(a.store.settings(), a.store.token()))) {
                    Json.put(a.store.root, "memories", Json.array(value, "memories"));
                    Json.put(a.store.root, "memoriesConnection", connection(settings, credential));
                    a.persist();
                  }
                } catch (Exception e) {
                  a.error(e);
                }
                if (dialog != null && dialog.isShowing()) render();
                if (issue != null) a.error(issue);
              });
        });
  }

  void invalidate(JSONObject conversation) {
    String id = conversation.optString("id");
    JSONArray memories = Json.array(a.store.root, "memories");
    for (int i = memories.length() - 1; i >= 0; i--) {
      JSONArray sources = Json.array(memories.optJSONObject(i), "sources");
      for (int j = 0; j < sources.length(); j++)
        if (sources.optJSONObject(j).optString("conversation").equalsIgnoreCase(id)) {
          memories.remove(i);
          break;
        }
    }
  }

  static JSONObject request(
      MainActivity a, JSONObject settings, String token, String action, JSONObject body)
      throws Exception {
    HttpUrl endpoint = Api.url(settings.optString("endpoint"));
    String path = endpoint.encodedPath();
    if (token.isEmpty() || !path.endsWith("/chat/completions"))
      throw new IOException("请先连接 Potato 服务，再开启记忆与历史。");
    String url =
        endpoint
            .newBuilder()
            .encodedPath(
                path.substring(0, path.length() - "chat/completions".length()) + "recall/" + action)
            .build()
            .toString();
    return a.api.json(url, token, body);
  }

  static synchronized JSONObject synchronize(
      MainActivity a, JSONObject settings, String token, JSONArray chats, boolean deletionsOnly)
      throws Exception {
    JSONObject status = request(a, settings, token, "status", null),
        entries = Json.object(status, "entries");
    String scope = hash(settings.optString("endpoint") + status.optString("scope"));
    android.util.AtomicFile file =
        new android.util.AtomicFile(new File(a.getFilesDir(), "recall-" + scope + ".json"));
    JSONObject acknowledged;
    try {
      acknowledged = new JSONObject(new String(file.readFully(), StandardCharsets.UTF_8));
    } catch (FileNotFoundException e) {
      acknowledged = Json.obj();
    }
    for (int i = 0; i < chats.length(); i++) {
      JSONObject c = chats.optJSONObject(i);
      if (c.optBoolean("example")) continue;
      boolean excluded = c.optBoolean("deleted") || c.optBoolean("recallExcluded");
      if (deletionsOnly && !excluded) continue;
      JSONArray messages = new JSONArray();
      if (!excluded)
        for (int j = 0; j < Json.array(c, "messages").length(); j++) {
          JSONObject m = Json.array(c, "messages").optJSONObject(j);
          if (!m.optString("state").equals("complete") || m.optString("text").isEmpty()) continue;
          JSONObject v =
              Json.obj(
                  "id",
                  m.optString("id").toLowerCase(Locale.ROOT),
                  "role",
                  m.optString("role"),
                  "text",
                  m.optString("text"),
                  "date",
                  Instant.ofEpochMilli(m.optLong("createdAt", c.optLong("created", 0))).toString());
          if (!messageVersion(m).isEmpty()) Json.put(v, "version", messageVersion(m));
          messages.put(v);
        }
      String id = c.optString("id").toLowerCase(Locale.ROOT);
      String content =
          canonical(
              Json.obj(
                  "id",
                  id,
                  "title",
                  c.optString("title").substring(0, Math.min(200, c.optString("title").length())),
                  "excluded",
                  excluded,
                  "messages",
                  messages));
      if (content.getBytes(StandardCharsets.UTF_8).length > 512000 || messages.length() > 2000)
        throw new IOException(
            "“" + c.optString("title") + "”超过历史同步上限（512 KB / 2000 条消息），请排除此对话后重试。");
      String digest = hash(content);
      JSONObject entry = entries.optJSONObject(id);
      if (entry != null && digest.equals(entry.optString("revision")))
        Json.put(acknowledged, id, digest);
      else if (entry != null || (!excluded && messages.length() > 0)) {
        JSONObject receipt =
            request(
                a,
                settings,
                token,
                "sync",
                Json.obj(
                    "content",
                    content,
                    "base",
                    acknowledged.has(id) ? acknowledged.optString(id) : JSONObject.NULL));
        Json.put(acknowledged, id, receipt.getString("revision"));
      }
      FileOutputStream out = null;
      try {
        out = file.startWrite();
        out.write(acknowledged.toString().getBytes(StandardCharsets.UTF_8));
        file.finishWrite(out);
      } catch (Exception e) {
        if (out != null) file.failWrite(out);
        throw e;
      }
    }
    return request(a, settings, token, "status", null);
  }

  static String hash(String s) throws Exception {
    StringBuilder b = new StringBuilder();
    for (byte v : MessageDigest.getInstance("SHA-256").digest(s.getBytes(StandardCharsets.UTF_8)))
      b.append(String.format(Locale.ROOT, "%02x", v & 255));
    return b.toString();
  }

  static String canonical(Object value) throws Exception {
    if (value instanceof JSONObject o) {
      ArrayList<String> keys = new ArrayList<>();
      o.keys().forEachRemaining(keys::add);
      Collections.sort(keys);
      ArrayList<String> parts = new ArrayList<>();
      for (String k : keys) parts.add(JSONObject.quote(k) + ":" + canonical(o.get(k)));
      return "{" + String.join(",", parts) + "}";
    }
    if (value instanceof JSONArray ar) {
      ArrayList<String> parts = new ArrayList<>();
      for (int i = 0; i < ar.length(); i++) parts.add(canonical(ar.get(i)));
      return "[" + String.join(",", parts) + "]";
    }
    return value instanceof String
        ? JSONObject.quote((String) value).replace("\\/", "/")
        : String.valueOf(value);
  }
}
