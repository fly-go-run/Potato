package top.recodex.potato;

import android.graphics.Typeface;
import android.view.*;
import android.widget.*;
import java.util.*;
import java.util.regex.*;
import org.json.*;

final class ConversationContent {
  static void render(MainActivity a, LinearLayout parent, JSONObject message) {
    String value = message.optString("text");
    if (value.isEmpty() && message.optString("state").equals("streaming")) {
      parent.addView(
          a.text(
              message.optString("reasoning").isEmpty() ? "正在准备回复…" : "正在思考…",
              14,
              MainActivity.MUTED));
      return;
    }
    Matcher code =
        Pattern.compile("(?m)^```([^\\n]*)\\n([\\s\\S]*?)(?:^```[ \\t]*$|\\z)").matcher(value);
    int last = 0;
    while (code.find()) {
      prose(a, parent, value.substring(last, code.start()));
      code(a, parent, code.group(1).trim(), code.group(2), message);
      last = code.end();
    }
    prose(a, parent, value.substring(last));
    recalls(a, parent, message);
    if (message.optJSONObject("execution") != null) {
      Button show =
          a.button(
              "云端计算",
              () -> {
                Json.put(message, "showExecution", !message.optBoolean("showExecution"));
                a.renderMessages();
              });
      parent.addView(show);
      if (message.optBoolean("showExecution")) {
        TextView output = a.text("", 14, MainActivity.INK);
        parent.addView(output);
        a.renderExecution(message.optJSONObject("execution"), output, parent);
      }
    }
  }

  static void prose(MainActivity a, LinearLayout parent, String text) {
    if (text.trim().isEmpty()) return;
    String[] lines = text.split("\n", -1);
    StringBuilder block = new StringBuilder();
    for (int i = 0; i < lines.length; i++) {
      if (i + 1 < lines.length
          && lines[i].contains("|")
          && lines[i + 1].matches("[\\s|:\\-]+")
          && lines[i + 1].contains("---")) {
        flush(a, parent, block);
        ArrayList<String> table = new ArrayList<>();
        table.add(lines[i]);
        i += 2;
        while (i < lines.length && lines[i].contains("|")) {
          table.add(lines[i]);
          i++;
        }
        i--;
        table(a, parent, table);
      } else block.append(lines[i]).append('\n');
    }
    flush(a, parent, block);
  }

  static void flush(MainActivity a, LinearLayout parent, StringBuilder b) {
    if (!b.toString().trim().isEmpty()) {
      TextView t = a.text("", 17, MainActivity.INK);
      a.markdown.setMarkdown(t, b.toString().strip());
      t.setTextIsSelectable(true);
      LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(-1, -2);
      p.bottomMargin = a.dp(12);
      parent.addView(t, p);
    }
    b.setLength(0);
  }

  static void code(
      MainActivity a, LinearLayout parent, String language, String source, JSONObject message) {
    LinearLayout card = a.column();
    card.setBackground(a.bg(MainActivity.CANVAS, 12));
    card.setClipToOutline(true);
    LinearLayout header = a.row();
    header.setPadding(a.dp(12), 0, a.dp(4), 0);
    header.addView(
        a.text(language.isEmpty() ? "代码" : language, 12, MainActivity.MUTED),
        new LinearLayout.LayoutParams(0, -2, 1));
    header.addView(a.icon("copy", "复制代码", () -> a.copy(source)));
    card.addView(header);
    View line = new View(a);
    line.setBackgroundColor(MainActivity.LINE);
    card.addView(line, new LinearLayout.LayoutParams(-1, a.dp(.5f)));
    HorizontalScrollView pan = new HorizontalScrollView(a);
    TextView code = a.text(source.stripTrailing(), 14, MainActivity.INK);
    code.setTypeface(Typeface.MONOSPACE);
    code.setTextIsSelectable(true);
    code.setHorizontallyScrolling(true);
    code.setPadding(a.dp(14), a.dp(14), a.dp(14), a.dp(14));
    pan.addView(code);
    card.addView(pan);
    if (language.equalsIgnoreCase("python")
        || language.equalsIgnoreCase("py")
        || language.equalsIgnoreCase("python3"))
      card.addView(a.button("运行 Python", () -> a.sandbox(message, source)));
    LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(-1, -2);
    p.setMargins(0, a.dp(8), 0, a.dp(20));
    parent.addView(card, p);
  }

  static void table(MainActivity a, LinearLayout parent, List<String> lines) {
    HorizontalScrollView pan = new HorizontalScrollView(a);
    TableLayout table = new TableLayout(a);
    for (int n = 0; n < lines.size(); n++) {
      TableRow row = new TableRow(a);
      String line = lines.get(n).trim().replaceAll("^\\||\\|$", "");
      for (String cell : line.split("\\|", -1)) {
        TextView t = a.text("", 15, MainActivity.INK);
        a.markdown.setMarkdown(t, cell.trim());
        t.setMinWidth(a.dp(90));
        t.setMaxWidth(a.dp(220));
        t.setPadding(a.dp(12), a.dp(12), a.dp(12), a.dp(12));
        android.graphics.drawable.GradientDrawable bg =
            a.bg(n == 0 ? MainActivity.SURFACE : MainActivity.CANVAS, 0);
        bg.setStroke(a.dp(.5f), MainActivity.LINE);
        t.setBackground(bg);
        if (n == 0) t.setTypeface(null, Typeface.BOLD);
        row.addView(t);
      }
      table.addView(row);
    }
    pan.addView(table);
    LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(-1, -2);
    p.bottomMargin = a.dp(16);
    parent.addView(pan, p);
  }

  static void recalls(MainActivity a, LinearLayout parent, JSONObject message) {
    JSONArray runs = Json.array(message, "recalls");
    ArrayList<JSONObject> sources = new ArrayList<>();
    HashSet<String> seen = new HashSet<>();
    for (int i = 0; i < runs.length(); i++) {
      JSONObject run = runs.optJSONObject(i);
      if (run.optString("state").equals("searching"))
        parent.addView(a.text("正在检索历史与记忆…", 13, MainActivity.MUTED));
      if (run.optString("state").equals("failed"))
        parent.addView(a.text(run.optString("message", "历史检索未完成"), 13, MainActivity.MUTED));
      JSONArray all = Json.array(run, "sources");
      for (int j = 0; j < all.length(); j++) {
        JSONObject source = all.optJSONObject(j),
            local = local(a, source.optString("conversation"));
        String key =
            source.optString("conversation")
                + "/"
                + source.optString("id")
                + "/"
                + source.optString("version");
        if ((local == null || (!local.optBoolean("deleted") && !local.optBoolean("recallExcluded")))
            && seen.add(key)) sources.add(source);
      }
    }
    if (sources.isEmpty()) return;
    parent.addView(
        a.button(
            "检索到的历史 · " + sources.size() + " 条",
            () -> {
              Json.put(message, "showRecalls", !message.optBoolean("showRecalls"));
              a.renderMessages();
            }));
    if (!message.optBoolean("showRecalls")) return;
    for (JSONObject source : sources) {
      TextView title = a.text(source.optString("title"), 15, MainActivity.INK);
      title.setTypeface(null, Typeface.BOLD);
      parent.addView(title);
      String date = source.optString("date");
      parent.addView(
          a.text(
              date.substring(0, Math.min(10, date.length()))
                  + " · "
                  + (source.optString("role").equals("user") ? "你" : "助手"),
              12,
              MainActivity.MUTED));
      TextView text = a.text(source.optString("text"), 14, MainActivity.INK);
      text.setMaxLines(5);
      text.setTextIsSelectable(true);
      parent.addView(text);
      parent.addView(a.button("查看原对话", () -> openSource(a, source)));
    }
  }

  static JSONObject local(MainActivity a, String id) {
    for (int i = 0; i < a.store.chats().length(); i++) {
      JSONObject c = a.store.chats().optJSONObject(i);
      if (c.optString("id").equalsIgnoreCase(id)) return c;
    }
    return null;
  }

  static void openSource(MainActivity a, JSONObject source) {
    JSONObject c = local(a, source.optString("conversation"));
    if (c == null || c.optBoolean("deleted") || c.optBoolean("recallExcluded")) {
      a.message("原对话不可用", "这条来源的原对话不在本机，或已被删除、排除。");
      return;
    }
    JSONArray messages = Json.array(c, "messages");
    int found = -1;
    for (int i = 0; i < messages.length(); i++)
      if (messages.optJSONObject(i).optString("id").equalsIgnoreCase(source.optString("id"))) {
        found = i;
        break;
      }
    if (found < 0) {
      a.message("原消息不可用", "这条来源的原消息已改变或不在本机。");
      return;
    }
    JSONObject original = messages.optJSONObject(found);
    if (!RecallScreen.messageVersion(original).equals(source.optString("version"))
        || !original.optString("text").startsWith(source.optString("text"))) {
      a.message("原消息已改变", "原消息内容或回复版本已改变，请重新检索。");
      return;
    }
    Json.put(a.store.root, "selected", c.optString("id"));
    Json.put(c, "documentOpen", false);
    a.follow = false;
    a.persist();
    a.showChat();
    final int index = found;
    a.scroll.post(
        () -> {
          View target = a.rows.getChildAt(index);
          if (target != null) {
            a.scroll.scrollTo(0, target.getTop());
            target.setBackground(a.bg(0xffeee8d7, 16));
          }
        });
  }
}
