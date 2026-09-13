package top.recodex.potato;

import android.graphics.Typeface;
import android.view.*;
import android.widget.*;
import org.json.*;

final class ExampleDocument {
  static final String TEXT =
      "# 给周末留一点空白\n\n"
          + "只安排两件想做的事，给临时起意留点余地。\n\n"
          + "## 周六 · 出门走走\n\n"
          + "- [ ] 去附近的公园走走，晒晒太阳\n"
          + "- [ ] 找一家喜欢的咖啡店，带上一本书\n\n"
          + "## 周日 · 慢慢收尾\n\n"
          + "- [ ] 整理一下房间，让下周更轻松\n"
          + "- [ ] 做一顿喜欢的饭，早点休息";

  static JSONObject create(Store store) {
    JSONObject c = store.newChat();
    Json.put(c, "title", "周末计划");
    Json.put(c, "example", true);
    Json.put(c, "sampleDocument", true);
    Json.put(c, "document", TEXT);
    Json.put(c, "documentOpen", true);
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
                "complete",
                "createdAt",
                System.currentTimeMillis()))
        .put(
            Json.obj(
                "id",
                Json.id(),
                "role",
                "assistant",
                "text",
                "整理成了一份可以继续修改的计划。",
                "state",
                "complete",
                "createdAt",
                System.currentTimeMillis()));
    return c;
  }

  static void render(MainActivity a, LinearLayout content, JSONObject chat) {
    TextView title = a.text("给周末留一点空白", 26, MainActivity.INK);
    title.setTypeface(null, Typeface.BOLD);
    title.setPadding(0, 0, 0, a.dp(10));
    content.addView(title);
    content.addView(a.text("周末计划 · 已保存在本机", 15, MainActivity.MUTED));
    View line = new View(a);
    line.setBackgroundColor(MainActivity.LINE);
    LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(-1, a.dp(.5f));
    p.setMargins(0, a.dp(10), 0, a.dp(10));
    content.addView(line, p);
    content.addView(a.text("只安排两件想做的事，给临时起意留点余地。", 17, MainActivity.INK));
    String[] lines = chat.optString("document").split("\n", -1);
    for (int i = 0; i < lines.length; i++) {
      String value = lines[i];
      if (value.startsWith("## ")) {
        TextView heading = a.text(value.substring(3), 20, MainActivity.INK);
        heading.setTypeface(null, Typeface.BOLD);
        heading.setPadding(0, a.dp(10), 0, a.dp(8));
        content.addView(heading);
      } else if (value.startsWith("- [")) {
        int index = i;
        CheckBox box = new CheckBox(a);
        box.setText(value.substring(6));
        box.setTextSize(17);
        box.setTextColor(value.charAt(3) == ' ' ? MainActivity.INK : MainActivity.MUTED);
        box.setMinHeight(a.dp(44));
        box.setChecked(value.charAt(3) != ' ');
        box.setButtonTintList(android.content.res.ColorStateList.valueOf(MainActivity.INK));
        box.setOnCheckedChangeListener(
            (v, on) -> {
              String[] current = chat.optString("document").split("\n", -1);
              current[index] = current[index].replaceFirst("\\[[ xX]]", on ? "[x]" : "[ ]");
              a.saveDocument(chat, String.join("\n", current));
              Json.put(chat, "sampleDocument", true);
              a.persist();
              box.setPaintFlags(
                  on
                      ? box.getPaintFlags() | android.graphics.Paint.STRIKE_THRU_TEXT_FLAG
                      : box.getPaintFlags() & ~android.graphics.Paint.STRIKE_THRU_TEXT_FLAG);
              box.setTextColor(on ? MainActivity.MUTED : MainActivity.INK);
            });
        content.addView(box);
      }
    }
  }
}
