package top.recodex.potato;

import android.app.AlertDialog;
import android.widget.*;
import java.util.*;
import org.json.*;

/** Same three-page selection flow and draft semantics as LocalModelPicker.swift. */
final class ModelPicker {
  final MainActivity a;
  final int retryIndex;
  final JSONObject choice;
  AlertDialog dialog;
  LinearLayout content;
  int page;

  ModelPicker(MainActivity a, int retryIndex) {
    this.a = a;
    this.retryIndex = retryIndex;
    choice =
        Json.copy(
            retryIndex < 0
                ? a.store.current()
                : Json.array(a.store.current(), "messages").optJSONObject(retryIndex));
    if (!choice.has("model")) Json.put(choice, "model", a.store.settings().optString("model"));
  }

  static String thinkingLabel(JSONObject c) {
    if (!c.optString("effort").isEmpty()) return SettingsScreen.effortName(c.optString("effort"));
    return switch (c.optString("thinking")) {
      case "disabled" -> "关闭";
      case "enabled" -> "开启";
      default -> "默认";
    };
  }

  void open() {
    show(0);
  }

  JSONArray catalog() {
    JSONArray all = Json.copy(a.store.settings()).optJSONArray("models");
    if (all == null) all = new JSONArray();
    String id = choice.optString("model");
    boolean found = false;
    for (int i = 0; i < all.length(); i++)
      if (all.optJSONObject(i).optString("id").equals(id)) found = true;
    if (!id.isEmpty() && !found && !a.store.settings().optBoolean("cloud"))
      all.put(Json.obj("id", id));
    return all;
  }

  void commit() {
    if (retryIndex < 0) {
      JSONObject c = a.store.current();
      for (String key : new String[] {"model", "thinking", "effort"}) {
        c.remove(key);
        if (choice.has(key)) Json.put(c, key, choice.opt(key));
      }
      Json.put(c, "endpoint", a.store.settings().optString("endpoint"));
      a.persist();
      a.model.setText(a.modelLabel());
    }
  }

  void choose(JSONObject m) {
    if (!choice.optString("model").equals(m.optString("id"))) {
      Json.put(choice, "model", m.optString("id"));
      choice.remove("thinking");
      choice.remove("effort");
    }
    commit();
    show(0);
  }

  void show(int next) {
    page = next;
    if (dialog != null) dialog.dismiss();
    content = a.column();
    content.setPadding(a.dp(20), a.dp(16), a.dp(20), a.dp(24));
    if (page != 0) content.addView(a.button("返回模型", () -> show(0)));
    String title = page == 1 ? "思考" : page == 2 ? "更多模型" : retryIndex < 0 ? "选择模型" : "换模型重新回答";
    dialog =
        ParitySheet.show(
            a,
            title,
            content,
            page == 0
                ? Math.min(660, (retryIndex < 0 ? 300 : 430) + Math.min(4, catalog().length()) * 64)
                : 0);
    if (page == 0) models();
    else if (page == 1) thinking();
    else more();
    if (retryIndex >= 0) {
      LinearLayout group = ParitySheet.group(a, content);
      Button confirm = a.button("重新回答", this::regenerate);
      confirm.setEnabled(
          !choice.optString("model").isEmpty()
              && a.generating == null
              && retryIndex == Json.array(a.store.current(), "messages").length() - 1);
      group.addView(confirm, new LinearLayout.LayoutParams(-1, a.dp(54)));
      content.addView(a.text("只重新回答这一条，保留旧回复；输入框的模型和草稿不变。", 13, MainActivity.MUTED));
    }
  }

  void modelRow(LinearLayout group, JSONObject m) {
    String id = m.optString("id"), name = m.optString("name", id);
    ParitySheet.item(
        a,
        group,
        name,
        a.store.settings().optBoolean("cloud") || id.equals(name) ? null : id,
        null,
        id.equals(choice.optString("model")),
        () -> choose(m));
  }

  void models() {
    JSONArray all = catalog();
    ArrayList<JSONObject> featured = new ArrayList<>();
    for (int i = 0; i < Math.min(4, all.length()); i++) featured.add(all.optJSONObject(i));
    for (int i = 4; i < all.length(); i++)
      if (all.optJSONObject(i).optString("id").equals(choice.optString("model"))) {
        featured.remove(featured.size() - 1);
        featured.add(0, all.optJSONObject(i));
      }
    LinearLayout group = ParitySheet.group(a, content);
    for (JSONObject m : featured) modelRow(group, m);
    if (featured.isEmpty()) group.addView(a.text("暂无可用模型，请在更多模型中添加。", 16, MainActivity.MUTED));
    if (!choice.optString("model").isEmpty()) {
      group = ParitySheet.group(a, content);
      ParitySheet.item(
          a, group, "思考", thinkingLabel(choice), "chevron-right", false, () -> show(1));
    }
    group = ParitySheet.group(a, content);
    ParitySheet.item(a, group, "更多模型", null, "chevron-right", false, () -> show(2));
  }

  void option(LinearLayout group, String label, String mode, String effort) {
    boolean checked =
        choice.optString("thinking").equals(mode == null ? "" : mode)
            && choice.optString("effort").equals(effort == null ? "" : effort);
    ParitySheet.item(
        a,
        group,
        label,
        null,
        null,
        checked,
        () -> {
          choice.remove("thinking");
          choice.remove("effort");
          if (mode != null) Json.put(choice, "thinking", mode);
          if (effort != null) Json.put(choice, "effort", effort);
          commit();
          show(1);
        });
  }

  void thinking() {
    JSONObject entry = a.settingsScreen.entry(choice.optString("model"));
    JSONArray modes = Json.array(entry, "thinking_modes"),
        efforts = Json.array(entry, "reasoning_effort_options");
    if (!Arrays.asList("", "effort").contains(entry.optString("thinking_param_style")))
      efforts = new JSONArray();
    boolean enabled = contains(modes, "enabled");
    LinearLayout group = ParitySheet.group(a, content);
    option(group, "服务默认", null, null);
    if (contains(modes, "disabled")) option(group, "关闭思考", "disabled", null);
    if (enabled && efforts.length() == 0) option(group, "开启思考", "enabled", null);
    for (int i = 0; i < efforts.length(); i++) {
      String effort = efforts.optString(i);
      option(group, SettingsScreen.effortName(effort), enabled ? "enabled" : null, effort);
    }
    content.addView(
        a.text(
            modes.length() == 0 && efforts.length() == 0
                ? "此模型未提供可选思考档位，将使用服务默认。"
                : "用于" + (retryIndex < 0 ? "下一次发送。" : "这次重新回答。") + "思考越深入，通常需要等待越久。",
            13,
            MainActivity.MUTED));
  }

  static boolean contains(JSONArray values, String value) {
    for (int i = 0; i < values.length(); i++) if (value.equals(values.optString(i))) return true;
    return false;
  }

  void more() {
    LinearLayout group = ParitySheet.group(a, content);
    EditText search = a.field("搜索模型", "");
    search.setSingleLine(true);
    group.addView(search);
    LinearLayout list = a.column();
    group.addView(list);
    Runnable render =
        () -> {
          list.removeAllViews();
          JSONArray all = catalog();
          String q = search.getText().toString().toLowerCase(Locale.ROOT);
          for (int i = 0; i < all.length(); i++) {
            JSONObject m = all.optJSONObject(i);
            if ((m.optString("name") + m.optString("id")).toLowerCase(Locale.ROOT).contains(q))
              modelRow(list, m);
          }
          if (list.getChildCount() == 0) list.addView(a.text("没有找到模型", 16, MainActivity.MUTED));
        };
    a.watch(search, s -> render.run());
    render.run();
    if (!a.store.settings().optBoolean("cloud")) {
      group = ParitySheet.group(a, content);
      EditText manual = a.field("服务提供的模型名称", choice.optString("model"));
      group.addView(manual);
      Button use =
          a.button(
              "使用此模型",
              () -> {
                String id = manual.getText().toString().trim();
                if (!id.isEmpty()) choose(Json.obj("id", id));
              });
      group.addView(use);
      use.setEnabled(manual.length() > 0);
      a.watch(manual, s -> use.setEnabled(!s.trim().isEmpty()));
    }
    group = ParitySheet.group(a, content);
    ParitySheet.item(
        a,
        group,
        "刷新模型列表",
        null,
        "refresh-cw",
        false,
        () -> a.settingsScreen.refreshModels(a.store.settings(), () -> show(2)));
    if (retryIndex < 0)
      ParitySheet.item(
          a,
          group,
          "连接设置",
          null,
          null,
          false,
          () -> {
            dialog.dismiss();
            a.settingsScreen.open();
          });
    if (a.store.settings().optBoolean("demo"))
      content.addView(a.text("本地体验模式，选择会保存但不调用模型。", 13, MainActivity.MUTED));
  }

  void regenerate() {
    JSONObject c = a.store.current(), before = Json.copy(c);
    for (String key : new String[] {"model", "thinking", "effort"}) {
      c.remove(key);
      if (choice.has(key)) Json.put(c, key, choice.opt(key));
    }
    Json.put(c, "endpoint", a.store.settings().optString("endpoint"));
    a.retryConfigured(retryIndex);
    for (String key : new String[] {"model", "thinking", "effort", "endpoint"}) {
      c.remove(key);
      if (before.has(key)) Json.put(c, key, before.opt(key));
    }
    a.persist();
    a.model.setText(a.modelLabel());
    dialog.dismiss();
  }
}
