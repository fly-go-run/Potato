package top.recodex.potato;

import android.app.*;
import android.content.*;
import android.widget.*;
import java.io.IOException;
import java.util.*;
import okhttp3.*;
import org.json.*;

final class SettingsScreen {
  final MainActivity a;
  boolean polling;
  AlertDialog dialog, loginDialog;
  TextView loginStatus;
  Button loginButton;

  SettingsScreen(MainActivity a) {
    this.a = a;
  }

  void open() {
    new SettingsForm(a, this).open();
  }

  void refreshModels(JSONObject settings, Runnable done) {
    try {
      String endpoint = settings.optString("endpoint"), token = a.store.token();
      HttpUrl u = Api.url(endpoint);
      if (!u.encodedPath().endsWith("/chat/completions")) throw new IOException("此地址无法读取模型列表。");
      String path =
          u.encodedPath().substring(0, u.encodedPath().length() - "/chat/completions".length())
              + "/models";
      a.async(
          () -> a.api.json(u.newBuilder().encodedPath(path).build().toString(), token, null),
          result -> {
            if (!endpoint.equals(a.store.settings().optString("endpoint"))
                || !token.equals(a.store.token())) return;
            installCatalog(settings, result);
            java.util.HashSet<String> available = new java.util.HashSet<>();
            JSONArray entries = Json.array(settings, "models");
            for (int i = 0; i < entries.length(); i++)
              available.add(entries.optJSONObject(i).optString("id"));
            for (int i = 0; i < a.store.chats().length(); i++) {
              JSONObject chat = a.store.chats().optJSONObject(i);
              if (chat.has("model") && !available.contains(chat.optString("model"))) {
                for (String key : new String[] {"model", "endpoint", "thinking", "effort"})
                  chat.remove(key);
              }
            }
            a.persist();
            done.run();
          });
    } catch (Exception e) {
      a.error(e);
    }
  }

  static void installCatalog(JSONObject settings, JSONObject result) throws Exception {
    JSONArray raw = result.optJSONArray("data");
    if (raw == null || raw.length() > 500) throw new IOException("模型列表格式无效。");
    JSONArray models = new JSONArray();
    HashSet<String> seen = new HashSet<>();
    for (int i = 0; i < raw.length(); i++) {
      JSONObject m = raw.optJSONObject(i);
      if (m == null) continue;
      String id = m.optString("id");
      if (id.trim().isEmpty() || id.length() > 256 || !seen.add(id)) continue;
      models.put(m);
    }
    if (models.length() == 0) throw new IOException("账号已登录，但没有可用模型。");
    Json.put(settings, "models", models);
    if (!seen.contains(settings.optString("model"))) {
      String id = result.optString("default_model");
      Json.put(settings, "model", seen.contains(id) ? id : models.optJSONObject(0).optString("id"));
    }
  }

  void login(String role) {
    JSONObject attempt = a.store.root.optJSONObject("login");
    if (attempt != null && !role.equals(attempt.optString("role"))) {
      a.message("登录进行中", "请先完成当前登录，再开启另一种连接。");
      return;
    }
    if (attempt != null) {
      showLogin(attempt);
      return;
    }
    String secret = (Json.id() + Json.id()).replace("-", "");
    if (loginButton != null) loginButton.setEnabled(false);
    a.worker.execute(
        () -> {
          try {
            JSONObject value =
                a.api.json(
                    Store.RELAY + "/v1/remote/auth/start",
                    "",
                    Json.obj("client_token", secret, "role", role, "name", "我的 Android · Potato"));
            HttpUrl verify = HttpUrl.parse(value.getString("verification_url"));
            HttpUrl root = Api.url(Store.RELAY);
            String id = value.getString("id");
            UUID.fromString(id);
            if (verify == null
                || !verify.isHttps()
                || !verify.host().equals(root.host())
                || verify.port() != root.port()
                || !verify.username().isEmpty()
                || !verify.password().isEmpty()
                || verify.fragment() != null
                || !verify.encodedPath().equals("/v1/remote/auth/authorize")
                || verify.querySize() != 1
                || !id.equals(verify.queryParameter("id"))) throw new IOException("登录服务返回无效地址。");
            a.store.secret("login:" + id, secret);
            Json.put(value, "role", role);
            a.main.post(
                () -> {
                  Json.put(a.store.root, "login", value);
                  a.persist();
                  if (loginButton != null) loginButton.setEnabled(true);
                  showLogin(value);
                });
          } catch (Exception e) {
            a.main.post(
                () -> {
                  if (loginButton != null) loginButton.setEnabled(true);
                  a.error(e);
                });
          }
        });
  }

  void showLogin(JSONObject attempt) {
    LinearLayout l = a.column();
    l.setPadding(a.dp(20), a.dp(10), a.dp(20), a.dp(10));
    TextView code = a.text(attempt.optString("code"), 30, MainActivity.INK);
    code.setTextIsSelectable(true);
    l.addView(code);
    l.addView(a.text("在浏览器登录获准的邮箱，核对验证码并确认，然后返回 Potato。", 15, MainActivity.MUTED));
    l.addView(a.button("打开登录页面", () -> a.openUrl(attempt.optString("verification_url"))));
    l.addView(a.button("我已登录，刷新", this::poll));
    loginDialog = a.sheet("核对登录验证码", l);
    l.addView(
        a.button(
            "取消本次登录",
            () -> {
              try {
                a.store.secret("login:" + attempt.optString("id"), "");
                a.store.root.remove("login");
                a.persist();
                loginDialog.dismiss();
              } catch (Exception e) {
                a.error(e);
              }
            }));
    poll();
    a.openUrl(attempt.optString("verification_url"));
  }

  void resume() {
    if (a.store.root.has("login")) poll();
  }

  void poll() {
    if (polling || !a.foreground) return;
    JSONObject attempt = a.store.root.optJSONObject("login");
    if (attempt == null) return;
    if (attempt.optLong("expires") < System.currentTimeMillis()) {
      a.store.root.remove("login");
      a.persist();
      a.toast("登录已过期，请重新开始");
      return;
    }
    polling = true;
    a.worker.execute(
        () -> {
          try {
            String id = attempt.optString("id"), secret = a.store.secret("login:" + id);
            JSONObject result =
                a.api.json(
                    Store.RELAY + "/v1/remote/auth/poll",
                    "",
                    Json.obj("id", id, "client_token", secret));
            a.main.post(
                () -> {
                  polling = false;
                  if (a.store.root.optJSONObject("login") != attempt) return;
                  try {
                    if (result.optString("status").equals("authorized")) {
                      String owner = result.optString("owner");
                      if (!owner.matches("[0-9a-fA-F]{64}")) throw new IOException("账号信息无效。");
                      String role = attempt.optString("role"),
                          token = owner + "." + id + "." + secret;
                      a.store.secret(role, token);
                      a.store.secret("login:" + id, "");
                      a.store.root.remove("login");
                      if (loginDialog != null) loginDialog.dismiss();
                      if (role.equals("cloud")) {
                        JSONObject s = a.store.settings();
                        Json.put(s, "endpoint", Store.RELAY + "/v1/chat/completions");
                        Json.put(s, "email", result.optString("email"));
                        Json.put(s, "cloud", true);
                        Json.put(s, "demo", false);
                        s.remove("models");
                        s.remove("model");
                        for (int i = 0; i < a.store.chats().length(); i++) {
                          JSONObject c = a.store.chats().optJSONObject(i);
                          for (String key :
                              new String[] {"model", "endpoint", "effort", "thinking"})
                            c.remove(key);
                        }
                        a.persist();
                        refreshModels(
                            s,
                            () -> {
                              if (loginStatus != null)
                                loginStatus.setText("已登录：" + result.optString("email"));
                              a.toast("云端模型已连接");
                              if (dialog != null) dialog.dismiss();
                              a.showChat();
                            });
                      } else {
                        Json.put(a.store.root, "remoteEmail", result.optString("email"));
                        a.persist();
                        a.remote.open();
                      }
                    } else a.main.postDelayed(this::poll, 2000);
                  } catch (Exception e) {
                    a.error(e);
                  }
                });
          } catch (Exception e) {
            a.main.post(
                () -> {
                  polling = false;
                  if (loginStatus != null) loginStatus.setText("登录查询失败，请返回后重试。");
                  a.main.postDelayed(this::poll, 5000);
                });
          }
        });
  }

  void logout() {
    try {
      String token = a.store.secret("cloud");
      a.async(
          () -> {
            try {
              return a.api.json(Store.RELAY + "/v1/remote/account/logout", token, Json.obj());
            } catch (Api.HttpFailure e) {
              if (e.status == 401) return Json.obj();
              throw e;
            }
          },
          result -> {
            a.store.secret("cloud", "");
            a.store.root.remove("memories");
            a.store.root.remove("memoriesEndpoint");
            JSONObject s = a.store.settings();
            s.remove("email");
            s.remove("models");
            s.remove("model");
            Json.put(s, "cloud", false);
            Json.put(s, "demo", true);
            a.persist();
            if (dialog != null) dialog.dismiss();
            a.showChat();
          });
    } catch (Exception e) {
      a.error(e);
    }
  }

  void models() {
    if (a.store.settings().optString("model").isEmpty()
        && Json.array(a.store.settings(), "models").length() == 0) open();
    else new ModelPicker(a, -1).open();
  }

  interface Pick {
    void choose(JSONObject model);
  }

  void showModels(Pick pick) {
    JSONObject s = a.store.settings();
    JSONArray models = s.optJSONArray("models");
    LinearLayout l = a.column();
    l.setPadding(a.dp(16), 0, a.dp(16), 0);
    AlertDialog d = a.sheet("模型与思考", l);
    if (models != null)
      for (int i = 0; i < models.length(); i++) {
        JSONObject m = models.optJSONObject(i);
        l.addView(
            a.button(
                m.optString("name", m.optString("id")),
                () -> {
                  d.dismiss();
                  pick.choose(m);
                }));
      }
    else
      l.addView(
          a.button(
              s.optString("model"),
              () -> {
                d.dismiss();
                pick.choose(Json.obj("id", s.optString("model")));
              }));
    l.addView(
        a.button(
            "思考设置",
            () -> {
              d.dismiss();
              JSONObject selected =
                  entry(a.store.current().optString("model", s.optString("model")));
              thinking(selected, a.store.current());
            }));
    l.addView(
        a.button(
            "刷新模型列表",
            () ->
                refreshModels(
                    s,
                    () -> {
                      d.dismiss();
                      showModels(pick);
                    })));
    l.addView(
        a.button(
            "连接设置",
            () -> {
              d.dismiss();
              open();
            }));
  }

  JSONObject entry(String id) {
    JSONArray models = a.store.settings().optJSONArray("models");
    if (models != null)
      for (int i = 0; i < models.length(); i++)
        if (models.optJSONObject(i).optString("id").equals(id)) return models.optJSONObject(i);
    return Json.obj("id", id);
  }

  void thinking(JSONObject model, JSONObject target) {
    LinearLayout l = a.column();
    l.setPadding(a.dp(14), 0, a.dp(14), 0);
    AlertDialog dialog = a.sheet("思考设置", l);
    l.addView(
        a.button(
            "服务默认",
            () -> {
              target.remove("effort");
              target.remove("thinking");
              a.persist();
              dialog.dismiss();
            }));
    JSONArray modes = model.optJSONArray("thinking_modes"),
        efforts = model.optJSONArray("reasoning_effort_options");
    if (modes != null)
      for (int i = 0; i < modes.length(); i++) {
        String mode = modes.optString(i);
        if (!Arrays.asList("enabled", "disabled").contains(mode)) continue;
        l.addView(
            a.button(
                mode.equals("enabled") ? "开启思考" : "关闭思考",
                () -> {
                  Json.put(target, "thinking", mode);
                  if (mode.equals("disabled")) target.remove("effort");
                  a.persist();
                  dialog.dismiss();
                }));
      }
    if (efforts != null
        && Arrays.asList("", "effort").contains(model.optString("thinking_param_style")))
      for (int i = 0; i < efforts.length(); i++) {
        String effort = efforts.optString(i);
        l.addView(
            a.button(
                effortName(effort),
                () -> {
                  Json.put(target, "effort", effort);
                  if (target.optString("thinking").equals("disabled")) target.remove("thinking");
                  a.persist();
                  dialog.dismiss();
                }));
      }
    if ((modes == null || modes.length() == 0) && (efforts == null || efforts.length() == 0))
      l.addView(a.text("当前服务未声明可选思考档位，使用服务默认。", 14, MainActivity.MUTED));
  }

  static String effortName(String e) {
    return switch (e) {
      case "none" -> "关闭";
      case "low" -> "低";
      case "medium" -> "中";
      case "high" -> "高";
      case "xhigh" -> "更高";
      case "max" -> "最高";
      case "ultra" -> "超高";
      default -> e;
    };
  }

  void retryModel(int index) {
    if (a.generating == null) new ModelPicker(a, index).open();
  }
}
