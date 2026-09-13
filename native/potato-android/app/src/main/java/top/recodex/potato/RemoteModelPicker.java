package top.recodex.potato;

import android.app.AlertDialog;
import android.widget.*;
import java.util.*;
import org.json.*;

final class RemoteModelPicker {
  final RemoteScreen r;
  final MainActivity a;
  JSONObject catalog;
  AlertDialog dialog;
  LinearLayout content;

  RemoteModelPicker(RemoteScreen r) {
    this.r = r;
    a = r.a;
    catalog = r.overview == null ? null : r.overview.optJSONObject("model_catalog");
  }

  void open() {
    show(0);
  }

  JSONObject effective() {
    return r.modelChoice != null
        ? r.modelChoice
        : catalog == null ? null : catalog.optJSONObject("active");
  }

  boolean matches(JSONObject m, JSONObject choice) {
    return choice != null
        && m.optString("provider_id").equals(choice.optString("provider_id"))
        && m.optString("id").equals(choice.optString("model"));
  }

  JSONObject entry() {
    if (catalog != null) {
      JSONArray models = Json.array(catalog, "models");
      for (int i = 0; i < models.length(); i++)
        if (matches(models.optJSONObject(i), effective())) return models.optJSONObject(i);
    }
    return null;
  }

  void save(JSONObject choice) {
    r.modelChoice = choice;
    if (choice == null) r.draft().remove("modelChoice");
    else Json.put(r.draft(), "modelChoice", Json.copy(choice));
    a.persist();
    r.updateControls();
  }

  void show(int page) {
    if (dialog != null) dialog.dismiss();
    content = a.column();
    content.setPadding(a.dp(20), a.dp(16), a.dp(20), a.dp(24));
    dialog =
        ParitySheet.show(
            a, page == 0 ? "选择模型" : page == 1 ? "思考" : "更多模型", content, page == 0 ? 530 : 0);
    if (page != 0) content.addView(a.button("返回模型", () -> show(0)));
    if (catalog == null || catalog.optInt("version") != 1) {
      content.addView(a.text("这台电脑尚不支持手机选择模型，请更新电脑端。发送时仍跟随电脑设置。", 15, MainActivity.MUTED));
      content.addView(a.button("重新读取", () -> reload(page)));
      return;
    }
    if (page == 0) {
      LinearLayout group = ParitySheet.group(a, content);
      ParitySheet.item(
          a,
          group,
          "跟随电脑",
          r.device.optString("name"),
          null,
          r.modelChoice == null,
          () -> {
            save(null);
            show(0);
          });
      JSONArray all = Json.array(catalog, "models");
      ArrayList<JSONObject> featured = new ArrayList<>();
      for (int i = 0; i < Math.min(3, all.length()); i++) featured.add(all.optJSONObject(i));
      for (int i = 3; i < all.length(); i++)
        if (matches(all.optJSONObject(i), effective())) {
          featured.remove(featured.size() - 1);
          featured.add(0, all.optJSONObject(i));
        }
      for (JSONObject m : featured) row(group, m);
      if (effective() != null && entry() != null) {
        group = ParitySheet.group(a, content);
        ParitySheet.item(
            a,
            group,
            "思考",
            effective().isNull("reasoning_effort")
                ? "服务默认"
                : SettingsScreen.effortName(effective().optString("reasoning_effort")),
            "chevron-right",
            false,
            () -> show(1));
      }
      group = ParitySheet.group(a, content);
      ParitySheet.item(a, group, "更多模型", null, "chevron-right", false, () -> show(2));
    } else if (page == 1) {
      JSONObject m = entry(), current = effective();
      if (m == null || current == null) return;
      LinearLayout group = ParitySheet.group(a, content);
      option(group, "服务默认", null, current);
      JSONArray efforts = Json.array(m, "effort_options");
      for (int i = 0; i < efforts.length(); i++)
        option(
            group, SettingsScreen.effortName(efforts.optString(i)), efforts.optString(i), current);
      content.addView(
          a.text(
              efforts.length() == 0 ? "这台电脑未提供可选档位。使用服务默认，或保留电脑已有配置。" : "用于下一轮任务。已发送的任务保留原配置。",
              13,
              MainActivity.MUTED));
    } else {
      LinearLayout group = ParitySheet.group(a, content);
      EditText query = a.field("搜索模型", "");
      query.setSingleLine(true);
      group.addView(query);
      LinearLayout list = a.column();
      group.addView(list);
      Runnable render =
          () -> {
            list.removeAllViews();
            JSONArray all = Json.array(catalog, "models");
            String q = query.getText().toString().toLowerCase(Locale.ROOT);
            for (int i = 0; i < all.length(); i++) {
              JSONObject m = all.optJSONObject(i);
              if ((m.optString("name") + m.optString("id") + m.optString("provider_name"))
                  .toLowerCase(Locale.ROOT)
                  .contains(q)) row(list, m);
            }
            if (list.getChildCount() == 0) list.addView(a.text("没有找到模型", 16, MainActivity.MUTED));
          };
      a.watch(query, s -> render.run());
      render.run();
      content.addView(a.button("刷新模型列表", () -> reload(2)));
    }
  }

  void row(LinearLayout group, JSONObject m) {
    ParitySheet.item(
        a,
        group,
        m.optString("name", m.optString("id")),
        m.optString("provider_name"),
        null,
        matches(m, r.modelChoice),
        () -> {
          if (!matches(m, r.modelChoice))
            save(
                Json.obj(
                    "provider_id",
                    m.optString("provider_id"),
                    "model",
                    m.optString("id"),
                    "reasoning_effort",
                    m.opt("default_effort")));
          show(0);
        });
  }

  void option(LinearLayout group, String name, String effort, JSONObject current) {
    boolean selected =
        effort == null
            ? current.isNull("reasoning_effort")
            : effort.equals(current.optString("reasoning_effort"));
    ParitySheet.item(
        a,
        group,
        name,
        null,
        null,
        selected,
        () -> {
          JSONObject next = Json.copy(current);
          Json.put(next, "reasoning_effort", effort == null ? JSONObject.NULL : effort);
          save(next);
          show(1);
        });
  }

  void reload(int page) {
    JSONObject device = r.device;
    a.async(
        () -> r.rpc(device, "overview", Json.obj(), Json.id()),
        result -> {
          if (r.device != device) return;
          r.overview = result;
          catalog = result.optJSONObject("model_catalog");
          if (dialog.isShowing()) show(page);
        });
  }
}
