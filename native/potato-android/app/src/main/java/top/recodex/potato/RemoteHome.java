package top.recodex.potato;

import android.graphics.*;
import android.view.*;
import android.widget.*;
import java.util.*;
import org.json.*;

/** Aggregate computer browser mirrors RemoteView: device chips, projects and task search. */
final class RemoteHome {
  final RemoteScreen r;
  final MainActivity a;
  final ArrayList<JSONObject> devices = new ArrayList<>();
  final Map<String, JSONObject> overviews = new HashMap<>();
  final Map<String, Boolean> online = new HashMap<>();
  LinearLayout list, chips;
  EditText search;
  String selected = "";
  long revision;

  RemoteHome(RemoteScreen r) {
    this.r = r;
    a = r.a;
  }

  void open() {
    r.visible = true;
    r.device = null;
    r.chat = null;
    r.pause();
    a.interruptVoice();
    a.base();
    LinearLayout bar = a.row();
    bar.setPadding(a.dp(18), a.dp(8), a.dp(18), a.dp(8));
    Button sidebar = a.icon("menu", "打开侧栏", a::sidebar);
    sidebar.setBackground(a.bg(0xeeffffff, 22));
    bar.addView(sidebar);
    TextView title = a.text("远程", 17, MainActivity.INK);
    title.setTypeface(null, Typeface.BOLD);
    title.setGravity(Gravity.CENTER);
    bar.addView(title, new LinearLayout.LayoutParams(0, -2, 1));
    Button more = a.icon("ellipsis", "远程选项", () -> {});
    more.setBackground(a.bg(0xeeffffff, 22));
    more.setOnClickListener(
        v -> {
          PopupMenu menu = new PopupMenu(a, more);
          for (String item : new String[] {"登录 Cloudflare 账号", "通过配对码添加电脑", "管理电脑", "刷新"})
            menu.getMenu().add(item);
          menu.setOnMenuItemClickListener(
              item -> {
                switch (item.getTitle().toString()) {
                  case "登录 Cloudflare 账号":
                    a.settingsScreen.login("phone");
                    break;
                  case "通过配对码添加电脑":
                    r.pair();
                    break;
                  case "管理电脑":
                    manage();
                    break;
                  default:
                    load();
                }
                return true;
              });
          menu.show();
        });
    bar.addView(more);
    a.root.addView(bar);
    HorizontalScrollView strip = new HorizontalScrollView(a);
    strip.setHorizontalScrollBarEnabled(false);
    chips = a.row();
    chips.setPadding(a.dp(20), a.dp(6), a.dp(20), a.dp(6));
    strip.addView(chips);
    a.root.addView(strip);
    ScrollView scroll = new ScrollView(a);
    list = a.column();
    list.setPadding(a.dp(24), 0, a.dp(24), a.dp(24));
    scroll.addView(list);
    a.root.addView(scroll, new LinearLayout.LayoutParams(-1, 0, 1));
    LinearLayout footer = a.row();
    footer.setPadding(a.dp(22), a.dp(8), a.dp(22), a.dp(12));
    search = a.field("搜索会话", "");
    search.setSingleLine(true);
    search.setTextSize(17);
    search.setBackground(a.bg(Color.WHITE, 24));
    search.setCompoundDrawablesWithIntrinsicBounds(R.drawable.lucide_search, 0, 0, 0);
    search.setCompoundDrawablePadding(a.dp(10));
    footer.addView(search, new LinearLayout.LayoutParams(0, a.dp(48), 1));
    a.watch(search, s -> render());
    for (boolean voice : new boolean[] {true, false}) {
      Button create =
          a.icon(voice ? "mic" : "square-pen", voice ? "语音指令" : "新远程任务", () -> compose(voice));
      create.setTextColor(Color.WHITE);
      create.setBackground(a.bg(MainActivity.INK, 23));
      LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(a.dp(46), a.dp(46));
      p.leftMargin = a.dp(10);
      footer.addView(create, p);
    }
    a.root.addView(footer);
    render();
    load();
  }

  boolean active() {
    return r.visible && r.device == null && r.home == this;
  }

  final Runnable poll = this::pollHome;

  void pollHome() {
    if (active() && a.foreground) load();
  }

  void load() {
    a.main.removeCallbacks(poll);
    a.main.postDelayed(poll, 5000);
    long epoch = ++revision;
    if (epoch == 1) {
      devices.clear();
      JSONArray saved = Json.array(a.store.root, "pairedDevices");
      for (int i = 0; i < saved.length(); i++) devices.add(saved.optJSONObject(i));
    }
    render();
    try {
      String token = a.store.secret("phone");
      if (token.isEmpty()) {
        refreshDevices(epoch);
        return;
      }
      a.worker.execute(
          () -> {
            try {
              JSONObject result =
                  a.api.json(Store.RELAY + "/v1/remote/account/devices", token, null);
              a.main.post(
                  () -> {
                    if (!active() || revision != epoch) return;
                    JSONArray values = Json.array(result, "devices");
                    devices.removeIf(d -> !d.optBoolean("paired"));
                    for (int i = 0; i < values.length(); i++) {
                      JSONObject d = values.optJSONObject(i);
                      Json.put(d, "relay", Store.RELAY);
                      boolean exists = false;
                      for (JSONObject local : devices)
                        if (r.targetKey(local, "").equals(r.targetKey(d, ""))) exists = true;
                      if (!exists) devices.add(d);
                    }
                    render();
                    refreshDevices(epoch);
                  });
            } catch (Exception e) {
              a.main.post(
                  () -> {
                    if (active() && revision == epoch) {
                      list.addView(a.text("连接中断 · " + e.getMessage(), 14, MainActivity.MUTED));
                      refreshDevices(epoch);
                    }
                  });
            }
          });
    } catch (Exception e) {
      a.error(e);
    }
  }

  void refreshDevices(long epoch) {
    for (JSONObject d : new ArrayList<>(devices))
      a.worker.execute(
          () -> {
            JSONObject overview = null;
            try {
              JSONObject status =
                  a.api.json(
                      d.optString("relay") + "/v1/remote/" + d.optString("id") + "/status",
                      r.credential(d),
                      null);
              if (status.optBoolean("online"))
                overview = r.rpc(d, "overview", Json.obj(), Json.id());
            } catch (Exception ignored) {
            }
            JSONObject result = overview;
            a.main.post(
                () -> {
                  if (!active() || revision != epoch) return;
                  online.put(d.optString("id"), result != null);
                  if (result != null) overviews.put(d.optString("id"), result);
                  render();
                });
          });
  }

  void render() {
    if (list == null) return;
    list.removeAllViews();
    chips.removeAllViews();
    if (devices.isEmpty()) {
      LinearLayout empty = a.column();
      empty.setGravity(Gravity.CENTER);
      empty.setPadding(0, a.dp(70), 0, 0);
      ImageView monitor = new ImageView(a);
      monitor.setImageResource(R.drawable.lucide_monitor);
      monitor.setColorFilter(MainActivity.MUTED);
      empty.addView(monitor, new LinearLayout.LayoutParams(a.dp(56), a.dp(56)));
      TextView title = a.text("让电脑上的任务，随你继续", 22, MainActivity.INK);
      title.setTypeface(null, Typeface.BOLD);
      title.setGravity(Gravity.CENTER);
      title.setPadding(0, a.dp(16), 0, a.dp(12));
      empty.addView(title);
      TextView note = a.text("电脑和手机登录同一个 Cloudflare 账号，已开启远程访问的电脑会显示在这里。", 15, MainActivity.MUTED);
      note.setGravity(Gravity.CENTER);
      empty.addView(note);
      Button login = a.button("登录 Cloudflare 账号", () -> a.settingsScreen.login("phone"));
      login.setTextColor(Color.WHITE);
      login.setTextSize(17);
      login.setTypeface(null, Typeface.BOLD);
      login.setBackground(a.bg(MainActivity.INK, 24));
      LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(-2, a.dp(48));
      lp.topMargin = a.dp(20);
      empty.addView(login, lp);
      empty.addView(a.button("通过配对码添加", r::pair));
      list.addView(empty);
      return;
    }
    chip("全部", "");
    for (JSONObject d : devices) chip(d.optString("name"), d.optString("id"));
    String q = search.getText().toString().toLowerCase(Locale.ROOT);
    ArrayList<JSONObject> visible = new ArrayList<>();
    for (JSONObject d : devices)
      if (selected.isEmpty() || selected.equals(d.optString("id"))) visible.add(d);
    for (JSONObject d : visible)
      if (!Boolean.TRUE.equals(online.get(d.optString("id")))) {
        list.addView(a.text(d.optString("name") + " · 离线", 16, MainActivity.INK));
        TextView note = a.text("保持电脑开机、联网并运行 Potato，连接恢复后即可继续任务。", 13, MainActivity.MUTED);
        note.setPadding(0, a.dp(6), 0, a.dp(18));
        list.addView(note);
      }
    for (String section : new String[] {"置顶", "项目", "会话"}) {
      LinearLayout rows = a.column();
      for (JSONObject d : visible) {
        JSONObject overview = overviews.get(d.optString("id"));
        if (overview == null) continue;
        JSONArray items = Json.array(overview, section.equals("项目") ? "projects" : "chats");
        for (int i = 0; i < items.length(); i++) {
          JSONObject item = items.optJSONObject(i);
          if (!item.optString("name").toLowerCase(Locale.ROOT).contains(q)) continue;
          if (!section.equals("项目") && item.optBoolean("pinned") != section.equals("置顶")) continue;
          LinearLayout line = a.row();
          Button main =
              a.button(
                  item.optString("name"),
                  () -> {
                    if (!Boolean.TRUE.equals(online.get(d.optString("id")))) return;
                    if (section.equals("项目")) project(d, item);
                    else enter(d, item, null, false);
                  });
          main.setGravity(Gravity.START | Gravity.CENTER_VERTICAL);
          main.setTextSize(17);
          main.setEnabled(Boolean.TRUE.equals(online.get(d.optString("id"))));
          if (section.equals("项目")) line.addView(a.icon("folder", "项目", () -> project(d, item)));
          LinearLayout names = a.column();
          names.addView(main);
          if (visible.size() > 1) {
            TextView owner = a.text(d.optString("name"), 12, MainActivity.MUTED);
            owner.setPadding(a.dp(12), 0, 0, a.dp(4));
            names.addView(owner);
          }
          line.addView(names, new LinearLayout.LayoutParams(0, -2, 1));
          if (section.equals("项目"))
            line.addView(
                a.icon(
                    "square-pen",
                    "在" + item.optString("name") + "新建任务",
                    () -> enter(d, null, item, false)));
          else if (item.optString("status").equals("running"))
            line.addView(a.text("执行中", 12, MainActivity.MUTED));
          rows.addView(line);
        }
      }
      if (rows.getChildCount() > 0) {
        TextView title = a.text(section, 17, MainActivity.INK);
        title.setTypeface(null, Typeface.BOLD);
        title.setPadding(0, a.dp(26), 0, a.dp(12));
        list.addView(title);
        list.addView(rows);
      }
    }
  }

  void chip(String title, String id) {
    Button b =
        a.button(
            title,
            () -> {
              selected = id;
              render();
            });
    b.setTextColor(selected.equals(id) ? Color.WHITE : MainActivity.INK);
    b.setBackground(a.bg(selected.equals(id) ? MainActivity.INK : MainActivity.SURFACE, 19));
    LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(-2, a.dp(38));
    p.rightMargin = a.dp(8);
    chips.addView(b, p);
  }

  void compose(boolean voice) {
    ArrayList<JSONObject> active = new ArrayList<>();
    for (JSONObject d : devices)
      if (Boolean.TRUE.equals(online.get(d.optString("id")))
          && (selected.isEmpty() || selected.equals(d.optString("id")))) active.add(d);
    if (active.isEmpty()) {
      if (devices.isEmpty()) a.settingsScreen.login("phone");
      else a.message("电脑离线", "连接恢复后即可继续任务。");
      return;
    }
    if (active.size() == 1) {
      enter(active.get(0), null, null, voice);
      return;
    }
    LinearLayout list = a.column();
    android.app.AlertDialog sheet = a.sheet("选择电脑", list);
    for (JSONObject d : active)
      ParitySheet.item(
          a,
          list,
          d.optString("name"),
          null,
          "monitor",
          false,
          () -> {
            sheet.dismiss();
            enter(d, null, null, voice);
          });
  }

  void enter(JSONObject d, JSONObject chat, JSONObject project, boolean voice) {
    r.device = d;
    r.overview = overviews.get(d.optString("id"));
    r.project = project == null ? "" : project.optString("path");
    r.task(chat);
    if (voice) a.main.postDelayed(r::startDictation, 200);
  }

  void project(JSONObject d, JSONObject p) {
    r.device = d;
    r.overview = overviews.get(d.optString("id"));
    r.shell(p.optString("name"), this::open);
    r.content.addView(a.button("新任务", () -> enter(d, null, p, false)));
    JSONArray tasks = Json.array(r.overview, "chats");
    for (int i = 0; i < tasks.length(); i++) {
      JSONObject c = tasks.optJSONObject(i);
      if (c.optString("project_path").equals(p.optString("path")))
        r.content.addView(a.button(c.optString("name"), () -> enter(d, c, p, false)));
    }
  }

  void manage() {
    LinearLayout content = a.column();
    content.setPadding(a.dp(20), a.dp(16), a.dp(20), a.dp(16));
    android.app.AlertDialog dialog = a.sheet("管理电脑", content);
    if (!a.store.root.optString("remoteEmail").isEmpty()) {
      LinearLayout account = ParitySheet.group(a, content);
      account.addView(a.text(a.store.root.optString("remoteEmail"), 17, MainActivity.INK));
      account.addView(a.button("退出这台手机的登录", () -> accountAction(null, dialog)));
    }
    for (JSONObject d : devices) {
      LinearLayout group = ParitySheet.group(a, content);
      group.addView(a.text(d.optString("name"), 17, MainActivity.INK));
      group.addView(
          a.text(
              Boolean.TRUE.equals(online.get(d.optString("id"))) ? "在线" : "离线",
              14,
              MainActivity.MUTED));
      boolean paired = d.optBoolean("paired");
      group.addView(
          a.text(
              paired ? "移除只清除本机配对。若要撤销旧手机访问，请在电脑重新生成配对码。" : "撤销后，所有手机都无法再控制这台电脑。电脑需要重新登录才能再次关联。",
              13,
              MainActivity.MUTED));
      Button remove =
          a.button(
              paired ? "从这台手机移除" : "撤销此电脑的远程访问",
              () -> {
                if (paired) {
                  try {
                    forget(d);
                    dialog.dismiss();
                    render();
                  } catch (Exception e) {
                    a.error(e);
                  }
                } else accountAction(d, dialog);
              });
      group.addView(remove);
    }
    content.addView(
        a.button(
            "添加电脑",
            () -> {
              dialog.dismiss();
              r.pair();
            }));
  }

  void forget(JSONObject d) throws Exception {
    if (!d.optBoolean("paired")) return;
    String key = r.targetKey(d, "");
    a.store.secret("device:" + key, "");
    JSONArray saved = Json.array(a.store.root, "pairedDevices");
    for (int i = saved.length() - 1; i >= 0; i--)
      if (r.targetKey(saved.optJSONObject(i), "").equals(key)) saved.remove(i);
    devices.removeIf(value -> r.targetKey(value, "").equals(key));
    online.remove(d.optString("id"));
    overviews.remove(d.optString("id"));
    if (selected.equals(d.optString("id"))) selected = "";
    revision++;
    a.persist();
  }

  void accountAction(JSONObject d, android.app.AlertDialog dialog) {
    try {
      String token = a.store.secret("phone");
      a.async(
          () -> {
            try {
              return a.api.json(
                  Store.RELAY + "/v1/remote/account/" + (d == null ? "logout" : "revoke"),
                  token,
                  d == null ? Json.obj() : Json.obj("device_id", d.optString("id")));
            } catch (Api.HttpFailure e) {
              if (d == null && e.status == 401) return Json.obj();
              throw e;
            }
          },
          result -> {
            if (!a.store.secret("phone").equals(token)) return;
            if (d == null) {
              a.store.secret("phone", "");
              a.store.root.remove("remoteEmail");
              devices.removeIf(value -> !value.optBoolean("paired"));
            } else devices.removeIf(value -> r.targetKey(value, "").equals(r.targetKey(d, "")));
            revision++;
            selected = "";
            a.persist();
            dialog.dismiss();
            render();
            load();
          });
    } catch (Exception e) {
      a.error(e);
    }
  }
}
