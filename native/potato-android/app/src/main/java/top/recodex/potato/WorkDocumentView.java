package top.recodex.potato;

import android.app.AlertDialog;
import android.graphics.Color;
import android.graphics.Typeface;
import android.view.*;
import android.widget.*;
import org.json.*;

/** Inline document keeps the chat composer available, with transactional editing. */
final class WorkDocumentView {
  final MainActivity a;
  final JSONObject chat;
  boolean expanded;
  LinearLayout panel;

  WorkDocumentView(MainActivity a, JSONObject c) {
    this.a = a;
    chat = c;
    expanded = c.optBoolean("documentExpanded");
  }

  void open() {
    a.input.setHint("想调整哪一部分？");
    Json.put(chat, "documentOpen", true);
    a.persist();
    render();
  }

  void close() {
    a.input.setHint("问问 Potato");
    Json.put(chat, "documentOpen", false);
    a.persist();
    a.stage.removeView(panel);
    a.scroll.setVisibility(View.VISIBLE);
    a.header.setVisibility(View.VISIBLE);
  }

  void render() {
    if (panel != null) a.stage.removeView(panel);
    a.scroll.setVisibility(View.GONE);
    a.header.setVisibility(expanded ? View.GONE : View.VISIBLE);
    panel = a.column();
    if (!expanded) {
      LinearLayout context = a.column();
      context.setPadding(a.dp(20), 0, a.dp(20), a.dp(18));
      JSONArray messages = Json.array(chat, "messages");
      if (messages.length() > 0) {
        TextView user = a.text(messages.optJSONObject(0).optString("text"), 17, MainActivity.INK);
        user.setMaxLines(2);
        user.setPadding(a.dp(16), a.dp(12), a.dp(16), a.dp(12));
        user.setBackground(a.bg(MainActivity.SURFACE, 22));
        LinearLayout.LayoutParams up = new LinearLayout.LayoutParams(-2, -2);
        up.gravity = Gravity.END;
        up.leftMargin = a.dp(36);
        context.addView(user, up);
        if (messages.length() > 1) {
          TextView reply =
              a.text(
                  messages.optJSONObject(messages.length() - 1).optString("text"),
                  17,
                  MainActivity.INK);
          reply.setMaxLines(2);
          context.addView(reply);
        }
      }
      context.setOnClickListener(v -> close());
      panel.addView(context);
    }
    LinearLayout card = a.column();
    card.setBackground(a.bg(Color.WHITE, 28));
    card.setClipToOutline(true);
    View handle = new View(a);
    handle.setBackground(a.bg(0xffdbdad6, 3));
    LinearLayout.LayoutParams hp = new LinearLayout.LayoutParams(a.dp(38), a.dp(5));
    hp.gravity = Gravity.CENTER_HORIZONTAL;
    hp.topMargin = a.dp(10);
    hp.bottomMargin = a.dp(8);
    card.addView(handle, hp);
    LinearLayout bar = a.row();
    bar.setPadding(a.dp(16), 0, a.dp(12), a.dp(8));
    Button documentIcon = a.icon("file-text", "工作文稿", () -> {});
    documentIcon.setBackground(a.bg(MainActivity.CANVAS, 12));
    bar.addView(documentIcon);
    TextView label = a.text("工作文稿", 17, MainActivity.INK);
    label.setTypeface(null, Typeface.BOLD);
    bar.addView(label, new LinearLayout.LayoutParams(0, -2, 1));
    bar.addView(a.icon("square-pen", "编辑文稿", this::edit));
    bar.addView(
        a.icon(
            "expand",
            expanded ? "收起文稿" : "展开文稿",
            () -> {
              expanded = !expanded;
              Json.put(chat, "documentExpanded", expanded);
              a.persist();
              render();
            }));
    bar.addView(a.icon("x", "关闭文稿", this::close));
    card.addView(bar);
    View border = new View(a);
    border.setBackgroundColor(MainActivity.LINE);
    card.addView(border, new LinearLayout.LayoutParams(-1, a.dp(.5f)));
    ScrollView scroll = new ScrollView(a);
    LinearLayout content = a.column();
    content.setPadding(a.dp(22), a.dp(14), a.dp(22), a.dp(14));
    renderMarkdown(content);
    scroll.addView(content);
    card.addView(scroll, new LinearLayout.LayoutParams(-1, 0, 1));
    View line = new View(a);
    line.setBackgroundColor(MainActivity.LINE);
    card.addView(line, new LinearLayout.LayoutParams(-1, a.dp(.5f)));
    LinearLayout footer = a.row();
    footer.setTag("document-footer");
    footer.setPadding(a.dp(16), a.dp(4), a.dp(16), a.dp(4));
    footer.addView(a.icon("copy", "复制文稿", () -> a.copy(chat.optString("document"))));
    footer.addView(a.button("复制", () -> a.copy(chat.optString("document"))));
    footer.addView(
        a.icon(
            "share",
            "分享文稿",
            () -> a.exportText(chat.optString("title") + ".md", chat.optString("document"))));
    footer.addView(
        a.button(
            "分享", () -> a.exportText(chat.optString("title") + ".md", chat.optString("document"))));
    footer.addView(new View(a), new LinearLayout.LayoutParams(0, 1, 1));
    footer.addView(a.icon("clock", "文稿历史", this::history));
    card.addView(footer);
    panel.addView(card, new LinearLayout.LayoutParams(-1, 0, 1));
    a.stage.addView(panel, new android.widget.FrameLayout.LayoutParams(-1, -1));
  }

  void renderMarkdown(LinearLayout content) {
    if (chat.optBoolean("sampleDocument")) {
      ExampleDocument.render(a, content, chat);
      return;
    }
    String[] lines = chat.optString("document").split("\n", -1);
    boolean code = false;
    StringBuilder block = new StringBuilder();
    for (int i = 0; i < lines.length; i++) {
      String line = lines[i], trim = line.trim();
      if (trim.startsWith("```")) code = !code;
      if (!code && trim.matches("[-*] \\[[ xX]] .*")) {
        flush(content, block);
        final int index = i;
        CheckBox box = new CheckBox(a);
        box.setButtonTintList(android.content.res.ColorStateList.valueOf(MainActivity.INK));
        box.setText(trim.substring(6));
        box.setTextSize(17);
        box.setTextColor(MainActivity.INK);
        box.setPadding(0, a.dp(10), 0, a.dp(10));
        box.setChecked(trim.charAt(3) != ' ');
        box.setOnCheckedChangeListener(
            (v, on) -> {
              String[] current = chat.optString("document").split("\n", -1);
              current[index] = current[index].replaceFirst("\\[[ xX]]", on ? "[x]" : "[ ]");
              a.saveDocument(chat, String.join("\n", current));
            });
        content.addView(box);
      } else block.append(line).append('\n');
    }
    flush(content, block);
  }

  void flush(LinearLayout parent, StringBuilder value) {
    if (value.toString().trim().isEmpty()) {
      value.setLength(0);
      return;
    }
    TextView t = a.text("", 17, MainActivity.INK);
    a.markdown.setMarkdown(t, value.toString());
    t.setTextIsSelectable(true);
    parent.addView(t);
    value.setLength(0);
  }

  void edit() {
    LinearLayout content = a.column();
    content.setPadding(a.dp(16), 0, a.dp(16), 0);
    EditText edit = a.field("编辑文稿", chat.optString("document"));
    edit.setBackgroundColor(MainActivity.CANVAS);
    edit.setTextSize(17);
    edit.setGravity(Gravity.TOP);
    edit.setMinLines(14);
    content.addView(edit);
    AlertDialog d = a.sheet("编辑文稿", content);
    d.setCanceledOnTouchOutside(false);
    ParitySheet.actions(
        a,
        d,
        "取消",
        d::dismiss,
        "保存",
        () -> {
          String value = edit.getText().toString();
          if (value.trim().isEmpty()) return;
          a.saveDocument(chat, value);
          d.dismiss();
          render();
        });
    LinearLayout bar = d.getWindow().getDecorView().findViewWithTag("parity-header");
    Button save = (Button) bar.getChildAt(2);
    save.setContentDescription("保存文稿编辑");
    save.setEnabled(!edit.getText().toString().trim().isEmpty());
    a.watch(edit, s -> save.setEnabled(!s.trim().isEmpty()));
  }

  void history() {
    LinearLayout list = a.column();
    list.setPadding(a.dp(20), a.dp(16), a.dp(20), a.dp(20));
    JSONArray versions = Json.array(chat, "documentVersions");
    AlertDialog d = a.sheet("文稿历史", list);
    if (versions.length() == 0) list.addView(a.text("还没有历史版本", 17, MainActivity.MUTED));
    for (int i = versions.length() - 1; i >= 0; i--) {
      JSONObject version = versions.optJSONObject(i);
      String text = version.optString("text");
      LinearLayout group = ParitySheet.group(a, list);
      String date =
          java.text.DateFormat.getDateTimeInstance()
              .format(new java.util.Date(version.optLong("time")));
      ParitySheet.item(
          a,
          group,
          date,
          text.substring(0, Math.min(100, text.length())),
          "clock",
          false,
          () -> {
            LinearLayout preview = a.column();
            TextView body = a.text("", 17, MainActivity.INK);
            a.markdown.setMarkdown(body, text);
            preview.addView(body);
            AlertDialog p = a.sheet("历史版本", preview);
            preview.addView(
                a.button(
                    "恢复此版本",
                    () -> {
                      a.saveDocument(chat, text);
                      Json.put(chat, "sampleDocument", version.optBoolean("sampleDocument"));
                      a.persist();
                      p.dismiss();
                      d.dismiss();
                      render();
                    }));
          });
    }
  }
}
