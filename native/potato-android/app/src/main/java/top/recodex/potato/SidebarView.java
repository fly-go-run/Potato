package top.recodex.potato;

import android.app.Dialog;
import android.graphics.Color;
import android.graphics.drawable.ColorDrawable;
import android.view.*;
import android.widget.*;
import java.util.*;
import org.json.*;

final class SidebarView extends Dialog {
  final MainActivity a;
  float startX;

  SidebarView(MainActivity activity) {
    super(activity);
    a = activity;
  }

  @Override
  public void show() {
    FrameLayout overlay = new FrameLayout(a);
    overlay.setBackgroundColor(0x11000000);
    int width =
        Math.min(a.dp(360), Math.round(a.getResources().getDisplayMetrics().widthPixels * .74f));
    LinearLayout panel = a.column();
    panel.setPadding(a.dp(20), a.dp(20), a.dp(12), a.dp(24));
    panel.setBackgroundColor(MainActivity.CANVAS);
    overlay.setOnClickListener(v -> dismiss());
    panel.setOnClickListener(v -> {});
    overlay.addView(panel, new FrameLayout.LayoutParams(width, -1, Gravity.START));
    LinearLayout header = a.row();
    TextView title = a.text("Potato", 22, MainActivity.INK);
    title.setTypeface(null, android.graphics.Typeface.BOLD);
    header.addView(title, new LinearLayout.LayoutParams(0, -2, 1));
    EditText query = a.field("搜索标题和消息", "");
    query.setSingleLine(true);
    query.setVisibility(View.GONE);
    header.addView(
        a.icon(
            "search",
            "搜索对话",
            () -> {
              query.setVisibility(query.getVisibility() == View.GONE ? View.VISIBLE : View.GONE);
              if (query.getVisibility() == View.VISIBLE) query.requestFocus();
            }));
    panel.addView(header);
    panel.addView(query);
    LinearLayout links = a.column();
    for (boolean remote : new boolean[] {false, true}) {
      LinearLayout item = a.row();
      item.setMinimumHeight(a.dp(48));
      Runnable open =
          () -> {
            dismiss();
            if (remote) a.remote.open();
            else a.history(false);
          };
      item.addView(a.icon(remote ? "monitor" : "book-open", remote ? "远程" : "资料库", open));
      TextView label = a.text(remote ? "远程" : "资料库", 17, MainActivity.INK);
      label.setTypeface(null, android.graphics.Typeface.BOLD);
      item.addView(label);
      item.setOnClickListener(v -> open.run());
      if (remote && a.remote.visible) item.setBackground(a.bg(MainActivity.SURFACE, 13));
      links.addView(item);
    }
    panel.addView(links);
    ScrollView scroll = new ScrollView(a);
    scroll.setVerticalScrollBarEnabled(false);
    LinearLayout list = a.column();
    scroll.addView(list);
    panel.addView(scroll, new LinearLayout.LayoutParams(-1, 0, 1));
    Runnable render =
        () -> {
          list.removeAllViews();
          String q = query.getText().toString().toLowerCase(Locale.ROOT);
          for (boolean pin : new boolean[] {true, false}) {
            ArrayList<JSONObject> chats = new ArrayList<>();
            for (int i = a.store.chats().length() - 1; i >= 0; i--) {
              JSONObject c = a.store.chats().optJSONObject(i);
              if (!c.optBoolean("deleted") && c.optBoolean("pinned") == pin && matches(c, q))
                chats.add(c);
            }
            chats.sort(
                (first, second) ->
                    Long.compare(
                        second.optLong("updated", second.optLong("created")),
                        first.optLong("updated", first.optLong("created"))));
            if (chats.isEmpty()) continue;
            TextView group = a.text(pin ? "置顶" : "最近对话", 17, MainActivity.INK);
            group.setTypeface(null, android.graphics.Typeface.BOLD);
            group.setPadding(a.dp(10), a.dp(26), 0, a.dp(12));
            list.addView(group);
            for (JSONObject c : chats) {
              Button b =
                  a.button(
                      c.optString("title"),
                      () -> {
                        dismiss();
                        Json.put(a.store.root, "selected", c.optString("id"));
                        a.persist();
                        a.follow = true;
                        a.showChat();
                      });
              b.setGravity(Gravity.CENTER_VERTICAL | Gravity.START);
              b.setTextSize(16);
              b.setSingleLine(true);
              b.setEllipsize(android.text.TextUtils.TruncateAt.END);
              if (c == a.store.current()) b.setBackground(a.bg(MainActivity.SURFACE, 13));
              b.setOnLongClickListener(
                  v -> {
                    PopupMenu menu = new PopupMenu(a, b);
                    menu.getMenu().add(c.optBoolean("pinned") ? "取消置顶" : "置顶");
                    menu.getMenu().add("删除");
                    menu.setOnMenuItemClickListener(
                        item -> {
                          if (item.getTitle().equals("删除")) {
                            Json.put(c, "deleted", true);
                            new RecallScreen(a).invalidate(c);
                            new RecallScreen(a).sync(true);
                            if (c == a.store.current()) a.store.newChat();
                          } else Json.put(c, "pinned", !c.optBoolean("pinned"));
                          a.persist();
                          dismiss();
                          a.showChat();
                          return true;
                        });
                    menu.show();
                    return true;
                  });
              list.addView(b, new LinearLayout.LayoutParams(-1, a.dp(48)));
            }
          }
        };
    a.watch(query, v -> render.run());
    render.run();
    LinearLayout footer = a.row();
    Button newChat =
        a.button(
            "对话",
            () -> {
              dismiss();
              a.beginNewChat();
            });
    newChat.setTextSize(17);
    newChat.setTextColor(Color.WHITE);
    newChat.setBackground(a.bg(MainActivity.INK, 24));
    newChat.setCompoundDrawablesWithIntrinsicBounds(R.drawable.lucide_square_pen, 0, 0, 0);
    for (android.graphics.drawable.Drawable drawable : newChat.getCompoundDrawables())
      if (drawable != null) drawable.setTint(Color.WHITE);
    footer.addView(newChat, new LinearLayout.LayoutParams(-2, a.dp(48)));
    footer.addView(new View(a), new LinearLayout.LayoutParams(0, 1, 1));
    Button settings =
        a.icon(
            "settings",
            "设置",
            () -> {
              dismiss();
              a.settingsScreen.open();
            });
    settings.setBackground(a.bg(Color.WHITE, 24));
    LinearLayout.LayoutParams sp = new LinearLayout.LayoutParams(a.dp(48), a.dp(48));
    sp.leftMargin = a.dp(12);
    footer.addView(settings, sp);
    panel.addView(footer);
    setContentView(overlay);
    super.show();
    Window w = getWindow();
    w.setBackgroundDrawable(new ColorDrawable(Color.TRANSPARENT));
    w.setLayout(-1, -1);
    w.clearFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND);
    w.getDecorView()
        .setSystemUiVisibility(
            View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
    w.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
    a.root.setBackground(a.bg(MainActivity.CANVAS, 30));
    a.root.setClipToOutline(true);
    a.root.animate().translationX(width).setDuration(220).start();
  }

  static boolean matches(JSONObject c, String q) {
    if (q.isEmpty() || c.optString("title").toLowerCase(Locale.ROOT).contains(q)) return true;
    JSONArray ms = Json.array(c, "messages");
    for (int i = 0; i < ms.length(); i++)
      if (ms.optJSONObject(i).optString("text").toLowerCase(Locale.ROOT).contains(q)) return true;
    return false;
  }

  @Override
  public boolean dispatchTouchEvent(MotionEvent e) {
    if (e.getAction() == MotionEvent.ACTION_DOWN) startX = e.getX();
    if (e.getAction() == MotionEvent.ACTION_UP && startX - e.getX() > a.dp(70)) {
      dismiss();
      return true;
    }
    return super.dispatchTouchEvent(e);
  }

  @Override
  public void dismiss() {
    a.root
        .animate()
        .translationX(0)
        .setDuration(180)
        .withEndAction(
            () -> {
              a.root.setClipToOutline(false);
              a.root.setBackgroundColor(MainActivity.CANVAS);
            })
        .start();
    super.dismiss();
  }
}
