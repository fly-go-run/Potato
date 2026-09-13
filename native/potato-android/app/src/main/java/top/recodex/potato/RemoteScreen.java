package top.recodex.potato;

import android.app.*;
import android.widget.*;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.*;
import okhttp3.HttpUrl;
import org.json.*;

final class RemoteScreen {
  final MainActivity a;
  RemoteHome home;
  Button sendControl, stopControl, modelControl;
  boolean processExpanded;
  boolean visible, busy, refreshing;
  JSONObject device, chat, snapshot, overview;
  LinearLayout content, messages;
  EditText input;
  TextView state;
  long confirmed;
  String project = "";
  JSONObject modelChoice;
  String renderedSnapshot = "";
  Voice dictation;
  Button voiceButton;

  RemoteScreen(MainActivity a) {
    this.a = a;
  }

  String targetKey(JSONObject d, String chatId) {
    return d.optString("relay") + "|" + d.optString("id") + "|" + chatId;
  }

  JSONObject draft() {
    return Json.object(
        Json.object(a.store.root, "remoteDrafts"),
        targetKey(device, chat == null ? "new:" + project : chat.optString("id")));
  }

  String credential(JSONObject d) throws Exception {
    return a.store.secret(d.optBoolean("paired") ? "device:" + targetKey(d, "") : "phone");
  }

  JSONObject rpc(JSONObject target, String op, JSONObject args, String id) throws Exception {
    return a.api
        .json(
            target.optString("relay") + "/v1/remote/" + target.optString("id") + "/rpc",
            credential(target),
            Json.obj("id", id, "op", op, "args", args))
        .getJSONObject("result");
  }

  void shell(String name, Runnable back) {
    a.base();
    LinearLayout bar = a.row();
    bar.addView(a.icon("‹", "返回", back));
    TextView title = a.text(name, 19, MainActivity.INK);
    bar.addView(title, new LinearLayout.LayoutParams(0, -2, 1));
    bar.addView(
        a.button(
            "刷新",
            () -> {
              if (device == null) open();
              else if (chat == null) device(device);
              else refresh();
            }));
    a.root.addView(bar);
    ScrollView scroll = new ScrollView(a);
    content = a.column();
    content.setPadding(a.dp(18), a.dp(12), a.dp(18), a.dp(18));
    scroll.addView(content);
    a.root.addView(scroll, new LinearLayout.LayoutParams(-1, 0, 1));
  }

  void open() {
    home = new RemoteHome(this);
    home.open();
  }

  void pair() {
    EditText edit = a.field("粘贴完整配对码", "");
    new AlertDialog.Builder(a)
        .setTitle("连接电脑")
        .setView(edit)
        .setNegativeButton("取消", null)
        .setPositiveButton(
            "连接",
            (dialog, which) -> {
              String code = edit.getText().toString().trim();
              a.async(
                  () -> {
                    HttpUrl url = HttpUrl.parse(code);
                    if (url == null
                        || !url.isHttps()
                        || !url.username().isEmpty()
                        || !url.password().isEmpty()
                        || url.query() != null
                        || url.fragment() == null
                        || !url.fragment().matches("[0-9a-fA-F]{64}"))
                      throw new IOException("请粘贴电脑生成的完整 HTTPS 配对码。");
                    List<String> parts = url.pathSegments();
                    if (parts.size() != 4
                        || !parts.get(0).equals("v1")
                        || !parts.get(1).equals("remote")
                        || !parts.get(3).equals("pair")) throw new IOException("配对码格式无效。");
                    UUID.fromString(parts.get(2));
                    String key =
                        "pair:"
                            + hex(
                                MessageDigest.getInstance("SHA-256")
                                    .digest(code.getBytes(StandardCharsets.UTF_8)));
                    String token = a.store.secret(key);
                    if (token.isEmpty()) {
                      token = (Json.id() + Json.id()).replace("-", "");
                      a.store.secret(key, token);
                    }
                    JSONObject result =
                        a.api.json(
                            url.newBuilder().fragment(null).build().toString(),
                            "",
                            Json.obj("pair_token", url.fragment(), "phone_token", token));
                    JSONObject d =
                        Json.obj(
                            "id",
                            parts.get(2),
                            "name",
                            result.optString("name"),
                            "relay",
                            url.newBuilder()
                                .encodedPath("/")
                                .fragment(null)
                                .build()
                                .toString()
                                .replaceAll("/$", ""),
                            "paired",
                            true);
                    a.store.secret("device:" + targetKey(d, ""), token);
                    return d;
                  },
                  d -> {
                    JSONArray devices = Json.array(a.store.root, "pairedDevices");
                    for (int i = devices.length() - 1; i >= 0; i--)
                      if (targetKey(devices.optJSONObject(i), "").equals(targetKey(d, "")))
                        devices.remove(i);
                    devices.put(d);
                    a.persist();
                    device(d);
                  });
            })
        .show();
  }

  static String hex(byte[] bytes) {
    StringBuilder s = new StringBuilder();
    for (byte b : bytes) s.append(String.format("%02x", b));
    return s.toString();
  }

  void device(JSONObject d) {
    visible = true;
    device = d;
    chat = null;
    snapshot = null;
    project = "";
    modelChoice = null;
    pause();
    shell(d.optString("name"), this::open);
    TextView loading = a.text("正在连接电脑…", 15, MainActivity.MUTED);
    content.addView(loading);
    a.async(
        () -> {
          JSONObject status =
              a.api.json(
                  d.optString("relay") + "/v1/remote/" + d.optString("id") + "/status",
                  credential(d),
                  null);
          if (!status.optBoolean("online")) throw new IOException("电脑当前离线，请打开 Potato 并检查远程访问。");
          return rpc(d, "overview", Json.obj(), Json.id());
        },
        result -> {
          if (device != d || !visible) return;
          overview = result;
          content.removeAllViews();
          content.addView(a.button("＋ 新任务", () -> task(null)));
          JSONArray projects = Json.array(result, "projects");
          if (projects.length() > 0) {
            content.addView(a.text("项目", 14, MainActivity.MUTED));
            for (int i = 0; i < projects.length(); i++) {
              JSONObject p = projects.optJSONObject(i);
              content.addView(
                  a.button(
                      p.optString("name"),
                      () -> {
                        project = p.optString("path");
                        task(null);
                      }));
            }
          }
          content.addView(a.text("电脑上的任务", 14, MainActivity.MUTED));
          JSONArray chats = Json.array(result, "chats");
          for (int i = 0; i < chats.length(); i++) {
            JSONObject c = chats.optJSONObject(i);
            content.addView(
                a.button(c.optString("name") + " · " + c.optString("status"), () -> task(c)));
          }
        });
  }

  void task(JSONObject c) {
    chat = c;
    snapshot = null;
    renderedSnapshot = "";
    confirmed = 0;
    visible = true;
    shell(c == null ? "新任务" : c.optString("name"), this::open);
    modelChoice = draft().optJSONObject("modelChoice");
    state =
        a.text(
            c == null
                ? device.optString("name") + " · " + (project.isEmpty() ? "默认工作区" : project)
                : "正在同步电脑状态…",
            13,
            MainActivity.MUTED);
    content.addView(state);
    messages = a.column();
    content.addView(messages);
    LinearLayout bottom = a.column();
    bottom.setPadding(a.dp(12), a.dp(12), a.dp(12), a.dp(12));
    android.graphics.drawable.GradientDrawable card = a.bg(android.graphics.Color.WHITE, 28);
    card.setStroke(a.dp(.8f), MainActivity.LINE);
    bottom.setBackground(card);
    input = a.field("发给 " + device.optString("name"), draft().optString("text"));
    input.setBackgroundColor(android.graphics.Color.TRANSPARENT);
    input.setTextSize(17);
    input.setMaxHeight(a.dp(180));
    bottom.addView(input);
    JSONObject saved = draft();
    a.watch(
        input,
        s -> {
          Json.put(saved, "text", s);
          a.scheduleSave();
          updateControls();
        });
    LinearLayout buttons = a.row();
    modelControl = a.button("跟随电脑", this::models);
    modelControl.setBackground(a.bg(MainActivity.CANVAS, 22));
    buttons.addView(modelControl);
    buttons.addView(new android.view.View(a), new LinearLayout.LayoutParams(0, 1, 1));
    voiceButton = a.icon("mic", "语音输入", this::startDictation);
    buttons.addView(voiceButton);
    sendControl = a.icon("arrow-up", "发送远程指令", this::send);
    sendControl.setTextColor(android.graphics.Color.WHITE);
    sendControl.setBackground(a.bg(MainActivity.INK, 22));
    buttons.addView(sendControl);
    stopControl = a.icon("square", "停止任务", this::stop);
    buttons.addView(stopControl);
    bottom.addView(buttons);
    LinearLayout.LayoutParams bp = new LinearLayout.LayoutParams(-1, -2);
    bp.setMargins(a.dp(10), a.dp(6), a.dp(10), a.dp(8));
    a.root.addView(bottom, bp);
    updateControls();
    JSONObject pending = saved.optJSONObject("pending");
    if (pending != null) {
      state.setText("上一条指令结果待确认，请核对后重试。");
      messages.addView(a.button("查看待确认指令", () -> pendingReview(saved, pending)));
    }
    if (Json.array(saved, "archived").length() > 0)
      messages.addView(a.button("历史回执", () -> archived(saved)));
    a.main.removeCallbacks(age);
    a.main.postDelayed(age, 1000);
    refresh();
  }

  final Runnable age = () -> ageState();

  void ageState() {
    if (!visible || chat == null || !a.foreground) return;
    if (!fresh() && snapshot != null) {
      state.setText("正在确认任务状态…");
      updateControls();
    }
    a.main.postDelayed(age, 1000);
  }

  boolean fresh() {
    return a.foreground && snapshot != null && System.currentTimeMillis() - confirmed < 10000;
  }

  void refresh() {
    if (!visible || chat == null || refreshing || !a.foreground) return;
    refreshing = true;
    JSONObject target = device, current = chat;
    a.worker.execute(
        () -> {
          try {
            JSONObject result =
                rpc(target, "chat", Json.obj("chat_id", current.optString("id")), Json.id());
            a.main.post(
                () -> {
                  refreshing = false;
                  if (!visible || device != target || chat != current || !a.foreground) return;
                  snapshot = result;
                  confirmed = System.currentTimeMillis();
                  render();
                  a.main.postDelayed(poll, 2000);
                });
          } catch (Exception e) {
            a.main.post(
                () -> {
                  refreshing = false;
                  if (!visible || device != target || chat != current) return;
                  confirmed = 0;
                  state.setText("连接中断，任务状态未确认");
                  updateControls();
                  a.main.postDelayed(poll, 5000);
                });
          }
        });
  }

  final Runnable poll = this::refresh;

  void pause() {
    if (dictation != null) {
      dictation.cancel();
      dictation = null;
      if (input != null) input.setEnabled(true);
    }
    a.main.removeCallbacks(poll);
    a.main.removeCallbacks(age);
    if (home != null) a.main.removeCallbacks(home.poll);
    confirmed = 0;
  }

  void resume() {
    if (visible && chat != null) {
      a.main.removeCallbacks(age);
      a.main.postDelayed(age, 1000);
      refresh();
    } else if (visible && device == null && home != null) home.load();
  }

  void startDictation() {
    if (!visible || input == null) return;
    if (dictation != null) {
      dictation.finish();
      voiceButton.setText("转写中");
      return;
    }
    if (a.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO)
        != android.content.pm.PackageManager.PERMISSION_GRANTED) {
      a.requestPermissions(new String[] {android.Manifest.permission.RECORD_AUDIO}, 43);
      return;
    }
    if (a.store.settings().optBoolean("demo")) {
      a.message("语音输入", "先在设置中登录云端模型，即可使用豆包语音。");
      return;
    }
    String original = input.getText().toString();
    final EditText editor = input;
    int start = Math.max(0, editor.getSelectionStart()),
        end = Math.max(start, editor.getSelectionEnd());
    try {
      String token = a.store.token();
      dictation =
          new Voice(
              a.api,
              new Voice.Listener() {
                public void text(String value, boolean finished) {
                  if (input != editor || dictation == null) return;
                  if (!value.isEmpty())
                    editor.setText(original.substring(0, start) + value + original.substring(end));
                  if (finished) {
                    dictation = null;
                    editor.setEnabled(true);
                    voiceButton.setText("语音");
                    a.persist();
                  }
                }

                public void level(double level) {
                  voiceButton.setText("完成录音");
                }

                public void error(String issue) {
                  dictation = null;
                  editor.setEnabled(true);
                  voiceButton.setText("语音");
                  a.persist();
                  a.message("语音输入", issue);
                }
              });
      editor.setEnabled(false);
      dictation.start(a.store.settings().optString("endpoint"), token);
    } catch (Exception e) {
      if (dictation != null) dictation.cancel();
      dictation = null;
      editor.setEnabled(true);
      a.error(e);
    }
  }

  void back() {
    if (device != null) open();
    else a.showChat();
  }

  void render() {
    if (snapshot == null) return;
    state.setText(device.optString("name") + " · " + remoteStatus());
    updateControls();
    String signature = snapshot.toString();
    if (signature.equals(renderedSnapshot) || messages.findFocus() instanceof EditText) return;
    renderedSnapshot = signature;
    messages.removeAllViews();
    LinkedHashMap<String, JSONObject> all = new LinkedHashMap<>();
    for (String key : new String[] {"messages", "live"}) {
      JSONArray list = Json.array(snapshot, key);
      for (int i = 0; i < list.length(); i++) {
        JSONObject m = list.optJSONObject(i);
        all.put(m.optString("id"), m);
      }
    }
    for (JSONObject m : all.values()) {
      String value = m.optString("text");
      if (value.isEmpty()) continue;
      LinearLayout block = a.column();
      block.setPadding(0, a.dp(10), 0, a.dp(10));
      String role = m.optString("role"), kind = m.optString("kind");
      boolean user = role.equals("user");
      boolean process =
          !user
              && !kind.isEmpty()
              && !Arrays.asList("text", "assistant", "reply", "message", "final").contains(kind);
      if (process) {
        Button toggle =
            a.button(
                processExpanded ? "收起过程" : "查看过程",
                () -> {
                  processExpanded = !processExpanded;
                  renderedSnapshot = "";
                  render();
                });
        block.addView(toggle);
        if (processExpanded) {
          TextView text = a.text(value, 14, MainActivity.MUTED);
          text.setTextIsSelectable(true);
          block.addView(text);
        }
      } else {
        if (user) {
          block.setBackground(a.bg(MainActivity.SURFACE, 22));
          block.setPadding(a.dp(16), a.dp(12), a.dp(16), a.dp(12));
          TextView text = a.text(value, 17, MainActivity.INK);
          block.addView(text);
        } else {
          ConversationContent.prose(a, block, value);
          LinearLayout actions = a.row();
          actions.addView(a.icon("copy", "复制回复", () -> a.copy(value)));
          actions.addView(
              a.icon(
                  "ellipsis",
                  "回复操作",
                  () -> {
                    LinearLayout options = a.column();
                    android.app.AlertDialog sheet = a.sheet("回复操作", options);
                    ParitySheet.item(
                        a,
                        options,
                        "分享回复",
                        null,
                        "share",
                        false,
                        () -> {
                          sheet.dismiss();
                          a.share(value);
                        });
                    ParitySheet.item(
                        a,
                        options,
                        "选择文字",
                        null,
                        "type",
                        false,
                        () -> {
                          sheet.dismiss();
                          LinearLayout selection = a.column();
                          TextView text = a.text(value, 17, MainActivity.INK);
                          text.setTextIsSelectable(true);
                          selection.addView(text);
                          a.sheet("选择文字", selection);
                        });
                  }));
          block.addView(actions);
        }
      }
      messages.addView(block);
    }
    JSONArray approvals = Json.array(snapshot, "approvals");
    for (int i = 0; i < approvals.length(); i++) {
      JSONObject approval = approvals.optJSONObject(i);
      LinearLayout card = a.column();
      card.setBackground(a.bg(MainActivity.SURFACE, 16));
      card.setPadding(a.dp(14), a.dp(12), a.dp(14), a.dp(12));
      card.addView(a.text("需要确认 · " + approval.optString("tool_name"), 17, MainActivity.INK));
      for (String key :
          new String[] {"findings_summary", "exact_target", "action_detail", "justification"})
        if (!approval.optString(key).isEmpty())
          card.addView(a.text(approval.optString(key), 14, MainActivity.INK));
      card.addView(
          a.button(
              "允许这一次",
              () ->
                  new AlertDialog.Builder(a)
                      .setTitle("允许电脑执行此操作？")
                      .setMessage(
                          approval.optString("action_detail", approval.optString("exact_target")))
                      .setNegativeButton("取消", null)
                      .setPositiveButton(
                          "允许",
                          (d, w) ->
                              act(
                                  "approval",
                                  Json.obj(
                                      "request_id",
                                      approval.optString("request_id"),
                                      "allow",
                                      true)))
                      .show()));
      card.addView(
          a.button(
              "拒绝",
              () ->
                  act(
                      "approval",
                      Json.obj("request_id", approval.optString("request_id"), "allow", false))));
      messages.addView(card);
    }
    JSONArray questions = Json.array(snapshot, "questions");
    for (int i = 0; i < questions.length(); i++) {
      JSONObject q = questions.optJSONObject(i);
      if (!q.optString("status").equals("pending")) continue;
      LinearLayout card = a.column();
      card.addView(a.text(q.optString("title"), 17, MainActivity.INK));
      JSONObject answerDraft =
          Json.object(
              Json.object(a.store.root, "remoteAnswers"),
              targetKey(device, chat.optString("id")) + "|" + q.optString("request_id"));
      HashSet<String> selected = new HashSet<>();
      JSONArray savedSelection = Json.array(answerDraft, "selected");
      for (int j = 0; j < savedSelection.length(); j++) selected.add(savedSelection.optString(j));
      ArrayList<CheckBox> boxes = new ArrayList<>();
      JSONArray options = Json.array(q, "options");
      for (int j = 0; j < options.length(); j++) {
        JSONObject o = options.optJSONObject(j);
        CheckBox check = new CheckBox(a);
        check.setText(o.optString("label"));
        boxes.add(check);
        check.setChecked(selected.contains(o.optString("id")));
        check.setOnCheckedChangeListener(
            (b, on) -> {
              if (on) {
                if (!q.optBoolean("multiple"))
                  for (CheckBox other : boxes) if (other != check) other.setChecked(false);
                selected.add(o.optString("id"));
              } else selected.remove(o.optString("id"));
              Json.put(answerDraft, "selected", new JSONArray(selected));
              a.scheduleSave();
            });
        card.addView(check);
      }
      EditText answer = a.field("补充回答", answerDraft.optString("text"));
      a.watch(
          answer,
          value -> {
            Json.put(answerDraft, "text", value);
            a.scheduleSave();
          });
      card.addView(answer);
      card.addView(
          a.button(
              "提交回答",
              () ->
                  act(
                      "answer",
                      Json.obj(
                          "request_id",
                          q.optString("request_id"),
                          "selected",
                          new JSONArray(selected),
                          "text",
                          answer.getText().toString(),
                          "skip",
                          false))));
      card.addView(
          a.button(
              "跳过",
              () ->
                  act(
                      "answer",
                      Json.obj(
                          "request_id",
                          q.optString("request_id"),
                          "selected",
                          new JSONArray(),
                          "text",
                          "",
                          "skip",
                          true))));
      messages.addView(card);
    }
    if (Json.array(draft(), "archived").length() > 0)
      messages.addView(a.button("历史回执", () -> archived(draft())));
    JSONObject pending = draft().optJSONObject("pending");
    if (pending != null)
      messages.addView(a.button("查看待确认指令", () -> pendingReview(draft(), pending)));
  }

  String remoteStatus() {
    if (snapshot == null) return "正在同步电脑状态…";
    String status = snapshot.optString("status");
    if (!fresh()) return "正在确认任务状态…";
    if (Json.array(snapshot, "approvals").length() > 0) return "等待你的批准";
    JSONArray questions = Json.array(snapshot, "questions");
    for (int i = 0; i < questions.length(); i++)
      if (questions.optJSONObject(i).optString("status").equals("pending")) return "等待你的回答";
    JSONObject outcome = snapshot.optJSONObject("outcome");
    if (outcome != null) {
      String result = outcome.optString("status");
      if (result.equals("failed")) return "本轮任务失败";
      if (result.equals("cancelled")) return "本轮任务已停止";
    }
    if (status.equals("running")) {
      JSONArray live = Json.array(snapshot, "live"), messages = Json.array(snapshot, "messages");
      JSONObject latest =
          live.length() > 0
              ? live.optJSONObject(live.length() - 1)
              : messages.length() > 0 ? messages.optJSONObject(messages.length() - 1) : null;
      if (latest != null) {
        String kind = latest.optString("kind");
        if (kind.equals("reasoning")) return "正在思考";
        if (kind.equals("tool") || kind.equals("tool_call") || kind.equals("function_call"))
          return "执行命令";
        if (kind.equals("message") && !latest.optString("text").isEmpty()) return "正在回复";
      }
      return "正在准备任务…";
    }
    return "本轮任务已完成";
  }

  void updateControls() {
    if (sendControl == null || stopControl == null || input == null || device == null) return;
    sendControl.setEnabled(
        !busy && input.length() > 0 && !draft().has("pending") && (chat == null || fresh()));
    sendControl.setBackground(a.bg(sendControl.isEnabled() ? MainActivity.INK : 0xffccccca, 22));
    boolean running = snapshot != null && snapshot.optString("status").equals("running");
    stopControl.setVisibility(running ? android.view.View.VISIBLE : android.view.View.GONE);
    stopControl.setEnabled(running && fresh() && !busy);
    if (modelControl != null)
      modelControl.setText(
          modelChoice == null
              ? "跟随电脑"
              : modelChoice.optString("model")
                  + " "
                  + SettingsScreen.effortName(modelChoice.optString("reasoning_effort", "默认")));
  }

  void models() {
    new RemoteModelPicker(this).open();
  }

  void send() {
    if (dictation != null) {
      a.toast("请先完成录音，核对文字后发送");
      return;
    }
    if (busy) return;
    JSONObject saved = draft();
    if (saved.has("pending")) {
      pendingReview(saved, saved.optJSONObject("pending"));
      return;
    }
    String text = input.getText().toString().trim();
    if (text.isEmpty()) return;
    if (chat != null && !fresh()) {
      a.message("请先刷新", "电脑状态尚未确认，草稿已保留。");
      refresh();
      return;
    }
    JSONObject args = Json.obj("text", text);
    if (chat != null) Json.put(args, "chat_id", chat.optString("id"));
    if (!project.isEmpty()) Json.put(args, "project_path", project);
    if (snapshot != null && snapshot.optString("status").equals("running")) {
      String run = snapshot.optString("running_request_id");
      if (run.isEmpty()) {
        a.message("无法安全追问", "请等待当前任务结束后刷新。");
        return;
      }
      Json.put(args, "expected_run_id", run);
    }
    JSONObject chosen = modelChoice;
    if (chosen == null && overview != null) {
      JSONObject catalog = overview.optJSONObject("model_catalog");
      if (catalog != null) chosen = catalog.optJSONObject("active");
    }
    if (chosen != null && overview != null) {
      JSONObject catalog = overview.optJSONObject("model_catalog");
      if (catalog != null) {
        JSONObject matching = null;
        JSONArray models = Json.array(catalog, "models");
        for (int i = 0; i < models.length(); i++) {
          JSONObject m = models.optJSONObject(i);
          if (m.optString("provider_id").equals(chosen.optString("provider_id"))
              && m.optString("id").equals(chosen.optString("model"))) matching = m;
        }
        if (matching == null) {
          a.message("模型不可用", "所选模型不可用，请重新选择或在电脑配置模型。");
          return;
        }
        String effort = chosen.optString("reasoning_effort", "");
        if (!effort.isEmpty()
            && !effort.equals("null")
            && !ModelPicker.contains(Json.array(matching, "effort_options"), effort)
            && !effort.equals(matching.optString("default_effort"))) {
          a.message("思考设置已改变", "请重新选择思考档位。");
          return;
        }
      }
    }
    if (chosen != null) Json.put(args, "model_choice", Json.copy(chosen));
    JSONObject pending = Json.obj("id", Json.id(), "target", targetKey(device, ""), "args", args);
    Json.put(saved, "pending", pending);
    try {
      a.store.save();
    } catch (Exception e) {
      saved.remove("pending");
      a.error(e);
      return;
    }
    transmit(saved, pending);
  }

  void pendingReview(JSONObject saved, JSONObject pending) {
    LinearLayout content = a.column();
    content.setPadding(a.dp(20), a.dp(16), a.dp(20), a.dp(24));
    content.addView(a.text(device.optString("name"), 17, MainActivity.INK));
    content.addView(a.text(pending.optJSONObject("args").optString("text"), 17, MainActivity.INK));
    content.addView(a.text("继续使用原操作编号请求回执，避免重复执行。", 14, MainActivity.MUTED));
    AlertDialog review = a.sheet("确认原指令结果", content);
    content.addView(
        a.button(
            "查询 / 重试原指令",
            () -> {
              review.dismiss();
              transmit(saved, pending);
            }));
    content.addView(
        a.button(
            "标记为待核对并归档",
            () -> {
              LinearLayout form = a.column();
              form.addView(
                  a.text("归档不会发送、取消或停止电脑任务。原指令与操作编号会保留，请先在电脑核对执行情况。", 16, MainActivity.INK));
              CheckBox checked = new CheckBox(a);
              checked.setText("我已核对目标电脑，理解任务状态仍未确认");
              form.addView(checked);
              AlertDialog confirmation = a.sheet("归档未确认指令", form);
              Button archive =
                  a.button(
                      "归档记录",
                      () -> {
                        JSONObject record = Json.copy(pending);
                        Json.put(record, "archivedAt", System.currentTimeMillis());
                        Json.array(saved, "archived").put(record);
                        saved.remove("pending");
                        a.persist();
                        confirmation.dismiss();
                        review.dismiss();
                        updateControls();
                      });
              archive.setEnabled(false);
              checked.setOnCheckedChangeListener((v, on) -> archive.setEnabled(on));
              form.addView(archive);
            }));
  }

  void archived(JSONObject saved) {
    LinearLayout content = a.column();
    content.setPadding(a.dp(20), a.dp(16), a.dp(20), a.dp(24));
    JSONArray entries = Json.array(saved, "archived");
    for (int i = entries.length() - 1; i >= 0; i--) {
      JSONObject record = entries.optJSONObject(i);
      LinearLayout group = ParitySheet.group(a, content);
      group.addView(a.text("任务状态未确认 · 已归档", 17, MainActivity.INK));
      group.addView(a.text(record.optJSONObject("args").optString("text"), 16, MainActivity.INK));
      group.addView(a.text("操作编号：" + record.optString("id"), 12, MainActivity.MUTED));
    }
    a.sheet("历史回执", content);
  }

  void transmit(JSONObject saved, JSONObject pending) {
    if (busy) return;
    if (!pending.optString("target").equals(targetKey(device, ""))) {
      a.message("电脑不匹配", "这条指令属于另一台电脑，未发送。");
      return;
    }
    JSONObject target = device;
    busy = true;
    updateControls();
    state.setText("正在确认发送结果…");
    a.worker.execute(
        () -> {
          try {
            JSONObject result =
                rpc(target, "send", pending.getJSONObject("args"), pending.optString("id"));
            a.main.post(
                () -> {
                  busy = false;
                  updateControls();
                  JSONObject next = result.optJSONObject("chat");
                  if (next == null) {
                    a.message("回执无效", "原指令已保留，请稍后核对。");
                    return;
                  }
                  saved.remove("pending");
                  // A receipt must not erase a follow-up typed while the request was in flight.
                  if (saved
                      .optString("text")
                      .trim()
                      .equals(pending.optJSONObject("args").optString("text")))
                    Json.put(saved, "text", "");
                  JSONObject destination =
                      Json.object(
                          Json.object(a.store.root, "remoteDrafts"),
                          targetKey(target, next.optString("id")));
                  if (destination != saved && destination.optString("text").isEmpty()) {
                    Json.put(destination, "text", saved.optString("text"));
                    Json.put(saved, "text", "");
                  }
                  a.persist();
                  if (visible && target == device) {
                    task(next);
                    a.toast(
                        result.optString("delivery").equals("recovered")
                            ? "已找回原指令，没有重复发送"
                            : "指令已送达电脑");
                  }
                });
          } catch (Exception e) {
            a.main.post(
                () -> {
                  busy = false;
                  if (visible && device == target) {
                    state.setText("结果待确认 · 原指令已保留");
                    updateControls();
                    a.error(e);
                  }
                });
          }
        });
  }

  void stop() {
    if (busy
        || !fresh()
        || snapshot.optInt("stop_protocol") != 1
        || !snapshot.optString("status").equals("running")) {
      a.message("无法停止", "请刷新并确认电脑仍在运行此任务。");
      return;
    }
    String run = snapshot.optString("running_request_id"), chatId = chat.optString("id");
    JSONObject target = device;
    if (run.isEmpty()) return;
    new AlertDialog.Builder(a)
        .setTitle("停止这次电脑任务？")
        .setMessage("仅停止当前看到的这一次运行。")
        .setNegativeButton("取消", null)
        .setPositiveButton(
            "停止",
            (d, w) -> {
              if (device != target || chat == null || !chat.optString("id").equals(chatId)) return;
              act("stop", Json.obj("expected_run_id", run));
            })
        .show();
  }

  void act(String op, JSONObject args) {
    if (busy || !fresh()) {
      a.message("状态已过期", "请刷新电脑状态后重试。");
      return;
    }
    JSONObject target = device, current = chat;
    Json.put(args, "chat_id", current.optString("id"));
    busy = true;
    a.worker.execute(
        () -> {
          try {
            rpc(target, op, args, Json.id());
            a.main.post(
                () -> {
                  busy = false;
                  if (visible && target == device && current == chat) refresh();
                });
          } catch (Exception e) {
            a.main.post(
                () -> {
                  busy = false;
                  a.error(e);
                  refresh();
                });
          }
        });
  }
}
