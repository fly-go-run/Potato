package top.recodex.potato;

import android.Manifest;
import android.app.*;
import android.content.*;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.*;
import android.speech.tts.TextToSpeech;
import android.text.*;
import android.view.*;
import android.view.inputmethod.InputMethodManager;
import android.widget.*;
import io.noties.markwon.Markwon;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.*;
import java.util.regex.*;
import okhttp3.Call;
import org.json.*;

public final class MainActivity extends Activity {
  static final int INK = 0xff1f1f1d,
      MUTED = 0xff757570,
      SURFACE = 0xfff0efea,
      LINE = 0xffe3e2df,
      CANVAS = 0xfff8f7f4;
  Store store;
  Api api;
  Attachments files;
  Markwon markdown;
  TextToSpeech tts;
  final ExecutorService worker = Executors.newFixedThreadPool(4);
  final Handler main = new Handler(Looper.getMainLooper());
  LinearLayout root, body, rows, composer, attachmentRow, voiceRow;
  ScrollView scroll;
  EditText input;
  TextView title, model, status;
  Button send;
  LinearLayout controls, header, overflow;
  FrameLayout stage;
  SidebarView drawer;
  TextView characterCount, attachmentCount;
  volatile Call chatCall;
  volatile JSONObject generating;
  JSONObject activeChat;
  boolean renderPending;
  boolean follow = true, rendering, foreground, storageAlert, importing;
  long lastSave;
  Voice voice;
  String voiceOriginal = "";
  int voiceStart, voiceEnd;
  boolean voiceSend, voiceReady, voiceFinishing;
  String voiceIdentity, voiceTranscript = "";
  Button voiceConfirm;
  VoiceWaveform waveform;
  TextView voiceHint;
  float gestureX, gestureY;
  boolean drawerGesture;
  RemoteScreen remote;
  SettingsScreen settingsScreen;

  interface Work<T> {
    T run() throws Exception;
  }

  interface Result<T> {
    void done(T value) throws Exception;
  }

  <T> void async(Work<T> work, Result<T> result) {
    worker.execute(
        () -> {
          try {
            T value = work.run();
            main.post(
                () -> {
                  if (isDestroyed()) return;
                  try {
                    result.done(value);
                  } catch (Exception e) {
                    error(e);
                  }
                });
          } catch (Exception e) {
            main.post(
                () -> {
                  if (!isDestroyed()) error(e);
                });
          }
        });
  }

  @Override
  public void onCreate(Bundle state) {
    super.onCreate(state);
    store = new Store(this);
    api = new Api();
    files = new Attachments(this);
    markdown =
        Markwon.builder(this)
            .usePlugin(
                new io.noties.markwon.AbstractMarkwonPlugin() {
                  @Override
                  public void configureTheme(io.noties.markwon.core.MarkwonTheme.Builder builder) {
                    builder
                        .headingBreakHeight(0)
                        .headingTextSizeMultipliers(
                            new float[] {22f / 17, 20f / 17, 18f / 17, 1, 1, 1});
                  }
                })
            .usePlugin(io.noties.markwon.ext.tables.TablePlugin.create(this))
            .usePlugin(io.noties.markwon.ext.strikethrough.StrikethroughPlugin.create())
            .build();
    tts = new TextToSpeech(this, s -> {});
    settingsScreen = new SettingsScreen(this);
    remote = new RemoteScreen(this);
    showChat();
    if (store.loadIssue != null) message("记录恢复", store.loadIssue);
  }

  @Override
  protected void onResume() {
    super.onResume();
    foreground = true;
    settingsScreen.resume();
    remote.resume();
  }

  @Override
  protected void onStop() {
    super.onStop();
    foreground = false;
    interruptVoice();

    persist();
    remote.pause();
  }

  @Override
  protected void onDestroy() {
    super.onDestroy();
    if (chatCall != null) chatCall.cancel();
    if (voice != null) voice.cancel();
    if (tts != null) tts.shutdown();
    worker.shutdownNow();
    main.removeCallbacksAndMessages(null);
  }

  int dp(float v) {
    return Math.round(v * getResources().getDisplayMetrics().density);
  }

  GradientDrawable bg(int color, int radius) {
    GradientDrawable d = new GradientDrawable();
    d.setColor(color);
    d.setCornerRadius(dp(radius));
    return d;
  }

  LinearLayout column() {
    LinearLayout l = new LinearLayout(this);
    l.setOrientation(LinearLayout.VERTICAL);
    return l;
  }

  LinearLayout row() {
    LinearLayout l = new LinearLayout(this);
    l.setGravity(Gravity.CENTER_VERTICAL);
    return l;
  }

  TextView text(String value, int size, int color) {
    TextView t = new TextView(this);
    t.setIncludeFontPadding(false);
    t.setText(value);
    t.setTextSize(size);
    t.setTextColor(color);
    t.setLineSpacing(dp(3), 1);
    return t;
  }

  Button button(String value, Runnable action) {
    Button b = new Button(this);
    b.setText(value);
    b.setTextSize(14);
    b.setAllCaps(false);
    b.setTextColor(INK);
    b.setMinHeight(dp(48));
    b.setMinimumHeight(dp(48));
    b.setMinWidth(dp(48));
    b.setMinimumWidth(dp(48));
    b.setPadding(dp(12), dp(4), dp(12), dp(4));
    b.setBackground(bg(Color.TRANSPARENT, 14));
    b.setStateListAnimator(null);
    b.setElevation(0);
    b.setContentDescription(value);
    b.setOnClickListener(
        v -> {
          if (store.settings().optBoolean("haptics", true))
            v.performHapticFeedback(HapticFeedbackConstants.CONTEXT_CLICK);
          action.run();
        });
    return b;
  }

  Button icon(String name, String label, Runnable action) {
    IconButton b = new IconButton(this, name);
    b.setContentDescription(label);
    b.setOnClickListener(
        v -> {
          if (store.settings().optBoolean("haptics", true))
            v.performHapticFeedback(HapticFeedbackConstants.CONTEXT_CLICK);
          action.run();
        });
    b.setLayoutParams(new LinearLayout.LayoutParams(dp(44), dp(44)));
    return b;
  }

  EditText field(String hint, String value) {
    EditText e = new EditText(this);
    e.setTextSize(16);
    e.setTextColor(INK);
    e.setHintTextColor(MUTED);
    e.setHint(hint);
    e.setText(value);
    e.setPadding(dp(14), dp(12), dp(14), dp(12));
    e.setBackground(bg(SURFACE, 14));
    e.setSingleLine(false);
    e.setContentDescription(hint);
    LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(-1, -2);
    p.setMargins(0, dp(7), 0, dp(7));
    e.setLayoutParams(p);
    return e;
  }

  void watch(EditText e, java.util.function.Consumer<String> changed) {
    e.addTextChangedListener(
        new TextWatcher() {
          public void beforeTextChanged(CharSequence s, int a, int c, int f) {}

          public void onTextChanged(CharSequence s, int start, int before, int count) {
            changed.accept(s.toString());
          }

          public void afterTextChanged(Editable e) {}
        });
  }

  void base() {
    root = column();
    root.setBackgroundColor(CANVAS);
    setContentView(root);
    if (Build.VERSION.SDK_INT >= 30) {
      getWindow().setDecorFitsSystemWindows(false);
      root.setOnApplyWindowInsetsListener(
          (v, insets) -> {
            android.graphics.Insets bars =
                insets.getInsets(
                    WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            android.graphics.Insets ime = insets.getInsets(WindowInsets.Type.ime());
            v.setPadding(bars.left, bars.top, bars.right, Math.max(bars.bottom, ime.bottom));
            if (input != null && !remote.visible) {
              float available =
                  (getResources().getDisplayMetrics().heightPixels
                          - bars.top
                          - Math.max(bars.bottom, ime.bottom))
                      / getResources().getDisplayMetrics().density;
              input.setMaxHeight(
                  dp(
                      Math.min(
                          voice == null ? 180 : 220,
                          Math.max(
                              72,
                              (available
                                      - 190
                                      - (Json.array(store.current(), "attachments").length() > 0
                                          ? 100
                                          : 0))
                                  * .55f))));
            }
            View footer = root.findViewWithTag("document-footer");
            if (footer != null)
              footer.setVisibility(ime.bottom > bars.bottom ? View.GONE : View.VISIBLE);
            return insets;
          });
    } else root.setFitsSystemWindows(true);
    getWindow()
        .getDecorView()
        .setSystemUiVisibility(
            View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
  }

  void persist() {
    try {
      store.save();
      storageAlert = false;
    } catch (Exception e) {
      if (!storageAlert) {
        storageAlert = true;
        message("保存失败", e.getMessage());
      }
    }
  }

  void scheduleSave() {
    if (remote != null && !remote.visible)
      Json.put(store.current(), "updated", System.currentTimeMillis());
    main.removeCallbacks(saveTask);
    main.postDelayed(saveTask, 300);
  }

  final Runnable saveTask = () -> persist();

  void error(Exception e) {
    message("暂时无法完成", e.getMessage() == null ? "请稍后重试。" : e.getMessage());
  }

  void message(String title, String value) {
    if (!isFinishing())
      new AlertDialog.Builder(this)
          .setTitle(title)
          .setMessage(value)
          .setPositiveButton("知道了", null)
          .show();
  }

  void toast(String value) {
    Toast.makeText(this, value, Toast.LENGTH_SHORT).show();
  }

  void hideKeyboard() {
    ((InputMethodManager) getSystemService(INPUT_METHOD_SERVICE))
        .hideSoftInputFromWindow(root.getWindowToken(), 0);
    root.clearFocus();
  }

  void showChat() {
    remote.visible = false;
    interruptVoice();
    base();
    LinearLayout bar = row();
    header = bar;
    bar.setPadding(dp(16), dp(6), dp(16), dp(6));
    Button menu = icon("menu", "打开侧栏", this::sidebar);
    menu.setBackground(bg(0xccffffff, 22));
    bar.addView(menu);
    title = text("Potato", 19, INK);
    title.setTypeface(null, Typeface.BOLD);
    title.setGravity(Gravity.CENTER);
    bar.addView(title, new LinearLayout.LayoutParams(0, -2, 1));
    bar.addView(
        icon(
            "square-pen",
            "新对话",
            () -> {
              beginNewChat();
            }));
    bar.getChildAt(bar.getChildCount() - 1).setBackground(bg(0xccffffff, 22));
    Button more = icon("ellipsis", "更多", this::moreMenu);
    more.setBackground(bg(0xccffffff, 22));
    LinearLayout.LayoutParams mp = new LinearLayout.LayoutParams(dp(44), dp(44));
    mp.leftMargin = dp(8);
    bar.addView(more, mp);
    root.addView(bar);
    scroll = new ScrollView(this);
    scroll.setFillViewport(true);
    scroll.setVerticalScrollBarEnabled(false);
    rows = column();
    rows.setPadding(dp(20), dp(20), dp(20), dp(20));
    scroll.addView(rows);
    stage = new FrameLayout(this);
    stage.addView(scroll);
    root.addView(stage, new LinearLayout.LayoutParams(-1, 0, 1));
    scroll.setOnTouchListener(
        (v, e) -> {
          if (e.getAction() == MotionEvent.ACTION_MOVE) follow = false;
          return false;
        });
    LinearLayout bottom = column();
    bottom.setPadding(dp(10), dp(6), dp(10), dp(8));
    status = text("", 12, MUTED);
    status.setGravity(Gravity.CENTER);
    status.setOnClickListener(
        v -> {
          follow = true;
          scroll.post(() -> scroll.fullScroll(View.FOCUS_DOWN));
        });
    bottom.addView(status);
    composer = column();
    GradientDrawable card = bg(Color.WHITE, 28);
    card.setStroke(dp(0.8f), LINE);
    composer.setBackground(card);
    composer.setElevation(0);
    composer.setPadding(dp(12), dp(12), dp(12), dp(12));
    attachmentCount = text("", 12, MUTED);
    composer.addView(attachmentCount);
    attachmentRow = row();
    HorizontalScrollView strip = new HorizontalScrollView(this);
    strip.addView(attachmentRow);
    composer.addView(strip);
    overflow = row();
    characterCount = text("", 12, MUTED);
    overflow.addView(characterCount, new LinearLayout.LayoutParams(0, -2, 1));
    overflow.addView(button("展开", this::expandInput));
    overflow.setVisibility(View.GONE);
    composer.addView(overflow);
    input = field("问问 Potato", store.current().optString("input"));
    input.setTextSize(17);
    input.setPadding(dp(6), dp(3), dp(6), dp(9));
    input.setLayoutParams(new LinearLayout.LayoutParams(-1, -2));
    input.setBackgroundColor(Color.TRANSPARENT);
    input.setMinLines(1);
    input.setMaxHeight(dp(180));
    input.setGravity(Gravity.TOP);
    input.setInputType(
        android.text.InputType.TYPE_CLASS_TEXT
            | android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE
            | android.text.InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);
    composer.addView(input);
    watch(
        input,
        s -> {
          if (!rendering) {
            Json.put(store.current(), "input", s);
            scheduleSave();
            updateComposer();
          }
        });
    controls = row();
    controls.addView(icon("＋", "添加照片或文件", this::pickFiles));
    model = text(modelLabel(), 15, INK);
    model.setPadding(dp(12), 0, dp(12), 0);
    model.setBackground(bg(CANVAS, 22));
    model.setMaxWidth(dp(230));
    model.setSingleLine(true);
    model.setEllipsize(android.text.TextUtils.TruncateAt.END);
    model.setMinHeight(dp(44));
    model.setGravity(Gravity.CENTER_VERTICAL);
    model.setOnClickListener(v -> settingsScreen.models());
    controls.addView(model, new LinearLayout.LayoutParams(-2, -2));
    controls.addView(new View(this), new LinearLayout.LayoutParams(0, 1, 1));
    controls.addView(icon("♩", "语音输入", this::startVoice));
    send =
        icon(
            "↑",
            "发送消息",
            () -> {
              if (generating != null) stopGeneration("已停止，部分内容已保留。");
              else send();
            });
    send.setTextColor(Color.WHITE);
    send.setBackground(bg(INK, 24));
    controls.addView(send);
    composer.addView(controls);
    voiceRow = row();
    composer.addView(voiceRow);
    bottom.addView(composer);
    root.addView(bottom);
    renderMessages();
    renderAttachments();
    if (store.current().optBoolean("documentOpen")
        && !store.current().optString("document").isEmpty()) document(store.current());
  }

  String modelLabel() {
    JSONObject c = store.current(), s = store.settings();
    if (s.optBoolean("demo") && c.optString("model", s.optString("model")).isEmpty()) return "本地体验";
    String id = c.optString("model", s.optString("model"));
    String name = id;
    JSONArray models = s.optJSONArray("models");
    if (models != null)
      for (int i = 0; i < models.length(); i++) {
        JSONObject m = models.optJSONObject(i);
        if (m.optString("id").equals(id)) name = m.optString("name", id);
      }
    return (name.isEmpty() ? "选择模型" : name) + "  " + ModelPicker.thinkingLabel(c);
  }

  void renderMessages() {
    if (remote.visible || rows == null || isDestroyed()) return;
    int oldY = scroll.getScrollY();
    rows.removeAllViews();
    JSONObject c = store.current();
    JSONArray messages = Json.array(c, "messages");
    title.setText(messages.length() == 0 ? "" : "Potato");
    if (messages.length() > 0 && store.settings().optBoolean("demo")) {
      android.text.SpannableString label = new android.text.SpannableString("Potato\n本地体验");
      label.setSpan(new android.text.style.AbsoluteSizeSpan(12, true), 7, label.length(), 0);
      label.setSpan(new android.text.style.ForegroundColorSpan(MUTED), 7, label.length(), 0);
      title.setText(label);
    }
    if (messages.length() == 0) {
      LinearLayout empty = column();
      empty.setGravity(Gravity.CENTER);
      ImageView image = new ImageView(this);
      image.setImageResource(R.drawable.potato_mark);
      image.setBackground(bg(CANVAS, 24));
      image.setClipToOutline(true);
      empty.addView(image, new LinearLayout.LayoutParams(dp(48), dp(48)));
      TextView welcome = text("今天，想聊什么？", 22, INK);
      welcome.setGravity(Gravity.CENTER);
      welcome.setTypeface(Typeface.create("sans-serif-medium", Typeface.NORMAL));
      welcome.setPadding(0, dp(18), 0, 0);
      empty.addView(welcome);
      rows.addView(empty, new LinearLayout.LayoutParams(-1, 0, 1));
    }
    for (int i = 0; i < messages.length(); i++) {
      JSONObject m = messages.optJSONObject(i);
      final int index = i;
      LinearLayout block = column();
      boolean user = m.optString("role").equals("user");
      LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(user ? -2 : -1, -2);
      if (user) p.gravity = Gravity.END;
      p.setMargins(user ? dp(36) : 0, 0, 0, dp(24));
      block.setLayoutParams(p);
      if (user) {
        block.setBackground(bg(SURFACE, 22));
        block.setPadding(dp(16), dp(12), dp(16), dp(12));
      }
      String reasoning = m.optString("reasoning");
      if (!reasoning.isEmpty()) {
        Button b =
            button(
                m.optBoolean("showReasoning") ? "收起思考过程  ⌃" : "思考过程  ﹀",
                () -> {
                  Json.put(m, "showReasoning", !m.optBoolean("showReasoning"));
                  renderMessages();
                });
        block.addView(b);
        if (m.optBoolean("showReasoning")) {
          TextView r = text(reasoning, 14, MUTED);
          r.setTextIsSelectable(true);
          block.addView(r);
        }
      }
      TextView content =
          text(
              m.optString("text").isEmpty() && m.optString("state").equals("streaming")
                  ? "正在思考…"
                  : m.optString("text"),
              17,
              INK);
      if (user) content.setMaxWidth(getResources().getDisplayMetrics().widthPixels - dp(108));
      content.setTextIsSelectable(true);
      if (!user) ConversationContent.render(this, block, m);
      else block.addView(content);
      JSONArray attachments = Json.array(m, "attachments");
      ArrayList<JSONObject> images = new ArrayList<>();
      for (int j = 0; j < attachments.length(); j++) {
        JSONObject attachment = attachments.optJSONObject(j);
        if (attachment.optString("type").startsWith("image/")) images.add(attachment);
        else block.addView(button(attachment.optString("name"), () -> openAttachment(attachment)));
      }
      if (!images.isEmpty()) {
        LinearLayout grid = column();
        int width = Math.min(dp(300), getResources().getDisplayMetrics().widthPixels - dp(108));
        for (int n = 0; n < images.size(); n += images.size() == 1 ? 1 : 2) {
          LinearLayout line = row();
          for (int col = 0; col < (images.size() == 1 ? 1 : 2) && n + col < images.size(); col++) {
            JSONObject attachment = images.get(n + col);
            ImageView picture = new ImageView(this);
            picture.setScaleType(ImageView.ScaleType.CENTER_CROP);
            try {
              picture.setImageURI(Uri.fromFile(files.file(attachment)));
            } catch (Exception ignored) {
            }
            picture.setBackground(bg(CANVAS, 12));
            picture.setClipToOutline(true);
            picture.setContentDescription(
                "预览 "
                    + attachment.optString("name")
                    + "，第 "
                    + (n + col + 1)
                    + " 张，共 "
                    + images.size()
                    + " 张");
            picture.setOnClickListener(v -> openAttachment(attachment));
            LinearLayout.LayoutParams ip =
                new LinearLayout.LayoutParams(
                    images.size() == 1 ? width : (width - dp(5)) / 2,
                    dp(images.size() == 1 ? 200 : 126));
            if (col > 0) ip.leftMargin = dp(5);
            line.addView(picture, ip);
          }
          LinearLayout.LayoutParams rp = new LinearLayout.LayoutParams(width, -2);
          rp.topMargin = dp(5);
          grid.addView(line, rp);
        }
        block.addView(grid);
      }
      JSONArray searches = Json.array(m, "searches");
      if (searches.length() > 0)
        block.addView(button("查看搜索来源（" + searches.length() + "）", () -> sources(m)));
      if (!m.optString("issue").isEmpty())
        block.addView(text(m.optString("issue"), 12, 0xffa34e26));
      if (!user && !m.optString("state").equals("streaming")) {
        LinearLayout actions = row();
        Button copy = icon("copy", "复制回复", () -> copy(m.optString("text")));
        copy.setOnClickListener(
            v -> {
              copy(m.optString("text"));
              ((IconButton) copy).symbol("check");
              copy.setContentDescription("已复制回复");
              main.postDelayed(
                  () -> {
                    ((IconButton) copy).symbol("copy");
                    copy.setContentDescription("复制回复");
                  },
                  2000);
            });
        actions.addView(copy);
        if (index == messages.length() - 1)
          actions.addView(icon("refresh-cw", "重新生成回复", () -> retry(index)));
        actions.addView(icon("ellipsis", "回复操作", () -> replyActions(m, index)));
        JSONArray versions = Json.array(m, "versions");
        if (versions.length() > 0) {
          int selectedVersion = m.optInt("versionIndex", versions.length());
          actions.addView(new View(this), new LinearLayout.LayoutParams(0, 1, 1));
          Button previous =
              icon("chevron-left", "上一版回复", () -> selectVersion(m, selectedVersion - 1));
          previous.setEnabled(selectedVersion > 0);
          actions.addView(previous);
          actions.addView(text((selectedVersion + 1) + " / " + (versions.length() + 1), 12, MUTED));
          Button next = icon("chevron-right", "下一版回复", () -> selectVersion(m, selectedVersion + 1));
          next.setEnabled(selectedVersion < versions.length());
          actions.addView(next);
        }
        block.addView(actions);
      } else if (user)
        content.setOnLongClickListener(
            v -> {
              editBranch(index);
              return true;
            });
      rows.addView(block);
    }
    if (!c.optString("document").isEmpty()) rows.addView(button("▤ 打开工作文稿", () -> document(c)));
    status.setText(generating != null && !follow ? "回到最新" : "");
    status.setVisibility(generating != null && !follow ? View.VISIBLE : View.GONE);
    updateComposer();
    model.setText(modelLabel());
    scroll.post(
        () -> {
          if (follow) scroll.fullScroll(View.FOCUS_DOWN);
          else scroll.scrollTo(0, oldY);
        });
  }

  void renderAttachments() {
    if (attachmentRow == null) return;
    attachmentRow.removeAllViews();
    JSONArray attachments = Json.array(store.current(), "attachments");
    attachmentCount.setText("附件 " + attachments.length() + " / 4 · 左右滑动查看");
    attachmentCount.setPadding(0, 0, 0, dp(8));
    attachmentCount.setVisibility(attachments.length() == 0 ? View.GONE : View.VISIBLE);
    for (int i = 0; i < attachments.length(); i++) {
      final int index = i;
      JSONObject file = attachments.optJSONObject(i);
      LinearLayout card = row();
      card.setBackground(bg(CANVAS, 12));
      card.setClipToOutline(true);
      if (file.optString("type").startsWith("image/")) {
        ImageView image = new ImageView(this);
        image.setScaleType(ImageView.ScaleType.CENTER_CROP);
        try {
          image.setImageURI(Uri.fromFile(files.file(file)));
        } catch (Exception ignored) {
        }
        image.setBackground(bg(CANVAS, 12));
        image.setClipToOutline(true);
        image.setContentDescription("预览 " + file.optString("name"));
        image.setOnClickListener(v -> openAttachment(file));
        card.addView(image, new LinearLayout.LayoutParams(dp(72), dp(72)));
      } else {
        TextView name = text(file.optString("name"), 12, INK);
        name.setMaxLines(2);
        name.setPadding(dp(10), 0, dp(10), 0);
        name.setOnClickListener(v -> openAttachment(file));
        card.addView(name, new LinearLayout.LayoutParams(dp(150), dp(72)));
      }
      Button remove =
          icon(
              "x",
              "移除附件 " + file.optString("name"),
              () -> {
                attachments.remove(index);
                persist();
                renderAttachments();
              });
      card.addView(remove);
      LinearLayout.LayoutParams cp = new LinearLayout.LayoutParams(-2, -2);
      cp.setMargins(0, 0, dp(8), dp(8));
      attachmentRow.addView(card, cp);
    }
    updateComposer();
  }

  void pickFiles() {
    if (importing) {
      toast("正在导入附件");
      return;
    }
    if (voice != null) interruptVoice();
    PopupMenu menu = new PopupMenu(this, controls.getChildAt(0));
    menu.getMenu().add("照片图库");
    menu.getMenu().add("选择文件");
    menu.setOnMenuItemClickListener(
        item -> {
          Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
          i.addCategory(Intent.CATEGORY_OPENABLE);
          i.setType(item.getTitle().equals("照片图库") ? "image/*" : "*/*");
          i.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
          startActivityForResult(i, 41);
          return true;
        });
    menu.show();
  }

  @Override
  protected void onActivityResult(int request, int result, Intent data) {
    super.onActivityResult(request, result, data);
    if (request != 41 || result != RESULT_OK || data == null) return;
    JSONObject target = store.current();
    ArrayList<Uri> uris = new ArrayList<>();
    if (data.getClipData() != null) {
      for (int i = 0; i < data.getClipData().getItemCount(); i++)
        uris.add(data.getClipData().getItemAt(i).getUri());
    } else if (data.getData() != null) uris.add(data.getData());
    if (uris.size() + Json.array(target, "attachments").length() > 4) {
      message("附件数量", "每条消息最多 4 个附件。");
      return;
    }
    importing = true;
    worker.execute(
        () -> {
          JSONArray imported = new JSONArray();
          Exception failure = null;
          try {
            for (Uri uri : uris) imported.put(files.importUri(uri));
          } catch (Exception e) {
            failure = e;
          }
          Exception issue = failure;
          main.post(
              () -> {
                importing = false;
                if (issue != null) {
                  error(issue);
                  return;
                }
                JSONArray a = Json.array(target, "attachments");
                for (int j = 0; j < imported.length(); j++) a.put(imported.optJSONObject(j));
                persist();
                if (target == store.current()) renderAttachments();
              });
        });
  }

  void send() {
    if (generating != null || importing) {
      toast("请等待当前操作完成");
      return;
    }
    JSONObject c = store.current();
    String value = input.getText().toString().trim();
    JSONArray a = Json.array(c, "attachments");
    if (value.isEmpty() && a.length() == 0) return;
    if (store.corrupt) {
      message("记录恢复", store.loadIssue);
      return;
    }
    if (!store.settings().optBoolean("demo") && store.settings().optString("model").isEmpty()) {
      settingsScreen.open();
      return;
    }
    JSONObject user =
        Json.obj(
            "id",
            Json.id(),
            "role",
            "user",
            "text",
            value,
            "attachments",
            Json.copy(a),
            "state",
            "complete");
    JSONArray messages = Json.array(c, "messages");
    Json.put(user, "createdAt", System.currentTimeMillis());
    messages.put(user);
    if (messages.length() == 1)
      Json.put(
          c, "title", value.isEmpty() ? "图片对话" : value.substring(0, Math.min(20, value.length())));
    Json.put(c, "input", "");
    Json.put(c, "attachments", new JSONArray());
    input.setText("");
    hideKeyboard();
    Json.put(c, "documentOpen", false);
    Json.put(c, "documentExpanded", false);
    stage.removeAllViews();
    stage.addView(scroll);
    scroll.setVisibility(View.VISIBLE);
    header.setVisibility(View.VISIBLE);
    input.setHint("问问 Potato");
    startReply(c);
    renderAttachments();
  }

  void startReply(JSONObject c) {
    JSONObject s = Json.copy(store.settings());
    String selected = c.optString("model", s.optString("model"));
    JSONObject reply =
        Json.obj(
            "id",
            Json.id(),
            "role",
            "assistant",
            "text",
            "",
            "reasoning",
            "",
            "state",
            "streaming",
            "model",
            selected,
            "effort",
            c.optString("effort"),
            "thinking",
            c.optString("thinking"),
            "endpoint",
            s.optString("endpoint"));
    JSONArray messages = Json.array(c, "messages"), history = Json.copy(messages);
    Json.put(reply, "createdAt", System.currentTimeMillis());
    messages.put(reply);
    generating = reply;
    activeChat = c;
    Json.put(c, "updated", System.currentTimeMillis());
    follow = true;
    persist();
    renderMessages();
    if (s.optBoolean("demo")) {
      main.postDelayed(
          () -> {
            if (generating != reply) return;
            Json.put(
                reply,
                "text",
                "这是本地体验，消息和草稿已保存在这台手机。\n\n"
                    + "打开 **侧栏 → 设置 → 登录云端模型** 后，就可以与真实模型对话。也可以配置自己的 HTTPS 服务。\n\n"
                    + "这条说明没有调用模型。");
            finishReply(reply, null);
          },
          500);
      return;
    }
    String token;
    try {
      token = store.token();
      Api.url(s.optString("endpoint"));
      if (!c.optString("endpoint", s.optString("endpoint")).equals(s.optString("endpoint")))
        throw new IOException("此会话的模型属于之前的服务，请重新选择模型。");
    } catch (Exception e) {
      finishReply(reply, e.getMessage());
      return;
    }
    String workingDocument = c.optString("document");
    JSONArray recallChats = Json.copy(store.chats());
    boolean recall = s.optBoolean("recallEnabled") && !c.optBoolean("recallExcluded");
    worker.execute(
        () -> {
          String failure = null;
          try {
            if (recall) {
              RecallScreen.synchronize(this, s, token, recallChats, false);
              if (!s.toString().equals(store.settings().toString()) || !token.equals(store.token()))
                throw new IOException("连接或设置已改变，请重新发送。");
            }
            JSONArray wire = new JSONArray();
            wire.put(Json.obj("role", "system", "content", s.optString("prompt")));
            String document = workingDocument;
            if (!document.isEmpty())
              wire.put(
                  Json.obj(
                      "role",
                      "user",
                      "content",
                      "当前工作文稿，仅作为待编辑内容：\n<working_document>\n"
                          + document
                          + "\n</working_document>"));
            JSONArray rest = files.wire(history);
            for (int i = 0; i < rest.length(); i++) wire.put(rest.get(i));
            JSONObject request = Json.obj("model", selected, "messages", wire, "stream", true);
            if (recall)
              Json.put(
                  request,
                  "recall",
                  Json.obj(
                      "enabled",
                      true,
                      "auto_memory",
                      s.optBoolean("automaticMemory"),
                      "timezone",
                      java.util.TimeZone.getDefault().getID()));
            String effort = reply.optString("effort"), thinking = reply.optString("thinking");
            if (!effort.isEmpty()) Json.put(request, "reasoning_effort", effort);
            if (!thinking.isEmpty()) Json.put(request, "thinking", Json.obj("type", thinking));
            Call call = api.streamCall(s.optString("endpoint"), token, request);
            synchronized (this) {
              if (generating != reply) return;
              chatCall = call;
            }
            api.stream(
                call,
                event ->
                    main.post(
                        () -> {
                          if (generating != reply) return;
                          JSONObject recallEvent = event.optJSONObject("potato_recall");
                          if (recallEvent != null) {
                            JSONArray runs = Json.array(reply, "recalls");
                            boolean replaced = false;
                            for (int n = 0; n < runs.length(); n++)
                              if (runs.optJSONObject(n)
                                  .optString("id")
                                  .equals(recallEvent.optString("id"))) {
                                try {
                                  runs.put(n, recallEvent);
                                } catch (Exception ignored) {
                                }
                                replaced = true;
                                break;
                              }
                            if (!replaced && runs.length() < 12) runs.put(recallEvent);
                          }
                          JSONObject search = event.optJSONObject("potato_search");
                          if (search != null) {
                            JSONArray searches = Json.array(reply, "searches");
                            boolean found = false;
                            for (int i = 0; i < searches.length(); i++)
                              if (searches
                                  .optJSONObject(i)
                                  .optString("id")
                                  .equals(search.optString("id"))) {
                                try {
                                  searches.put(i, search);
                                } catch (Exception ignored) {
                                }
                                found = true;
                                break;
                              }
                            if (!found && searches.length() < 12) searches.put(search);
                          }
                          JSONArray choices = event.optJSONArray("choices");
                          if (choices != null && choices.length() > 0) {
                            JSONObject delta = choices.optJSONObject(0).optJSONObject("delta");
                            if (delta != null) {
                              Json.put(
                                  reply,
                                  "text",
                                  reply.optString("text") + delta.optString("content", ""));
                              Json.put(
                                  reply,
                                  "reasoning",
                                  reply.optString("reasoning")
                                      + delta.optString("reasoning_content", ""));
                            }
                          }
                          if (!renderPending) {
                            renderPending = true;
                            main.postDelayed(renderTask, 70);
                          }
                          if (System.currentTimeMillis() - lastSave > 1500) {
                            persist();
                            lastSave = System.currentTimeMillis();
                          }
                        }));
          } catch (Exception e) {
            failure = e.getMessage();
          }
          String issue = failure;
          main.post(() -> finishReply(reply, issue));
        });
  }

  final Runnable renderTask =
      () -> {
        renderPending = false;
        renderMessages();
      };

  void finishReply(JSONObject reply, String issue) {
    if (generating != reply) return;
    if (issue == null && reply.optString("text").trim().isEmpty()) issue = "服务没有返回文字，请检查模型设置后重试。";
    Json.put(
        reply,
        "state",
        issue == null ? "complete" : reply.optString("text").isEmpty() ? "failed" : "interrupted");
    if (issue != null) Json.put(reply, "issue", issue);
    generating = null;
    activeChat = null;
    chatCall = null;
    persist();
    renderMessages();
  }

  void stopGeneration(String reason) {
    JSONObject reply = generating;
    if (reply == null) return;
    if (chatCall != null) chatCall.cancel();
    finishReply(reply, reason);
  }

  void retry(int index) {
    if (generating != null) return;
    JSONObject c = store.current(),
        m = Json.array(c, "messages").optJSONObject(index),
        previous = Json.copy(c);
    for (String key : new String[] {"model", "endpoint", "thinking", "effort"}) {
      if (m.has(key)) Json.put(c, key, m.opt(key));
      else c.remove(key);
    }
    retryConfigured(index);
    for (String key : new String[] {"model", "endpoint", "thinking", "effort"}) {
      c.remove(key);
      if (previous.has(key)) Json.put(c, key, previous.opt(key));
    }
    persist();
  }

  void retryConfigured(int index) {
    if (generating != null) return;
    JSONObject c = store.current();
    JSONArray messages = Json.array(c, "messages");
    JSONObject original = messages.optJSONObject(index);
    if (index < messages.length() - 1) {
      JSONObject branch = branch(c, index);
      startReply(branch);
      showChat();
      return;
    }
    JSONObject saved = Json.copy(original);
    saved.remove("versions");
    JSONArray versions = Json.copy(Json.array(original, "versions"));
    versions.put(saved);
    messages.remove(index);
    startReply(c);
    Json.put(
        generating == null ? Json.array(c, "messages").optJSONObject(index) : generating,
        "versions",
        versions);
    persist();
  }

  JSONObject branch(JSONObject c, int before) {
    JSONObject newChat = store.newChat();
    Json.put(newChat, "title", c.optString("title") + " · 分支");
    for (String key : new String[] {"model", "endpoint", "effort", "thinking", "document"})
      if (c.has(key)) Json.put(newChat, key, c.opt(key));
    JSONArray dest = Json.array(newChat, "messages"), source = Json.array(c, "messages");
    for (int i = 0; i < before; i++) dest.put(Json.copy(source.optJSONObject(i)));
    return newChat;
  }

  void editBranch(int index) {
    JSONObject c = store.current();
    EditText edit = field("修改消息", Json.array(c, "messages").optJSONObject(index).optString("text"));
    new AlertDialog.Builder(this)
        .setTitle("编辑为新分支")
        .setView(edit)
        .setNegativeButton("取消", null)
        .setPositiveButton(
            "创建分支",
            (d, w) -> {
              JSONObject b = branch(c, index);
              JSONObject original = Json.array(c, "messages").optJSONObject(index);
              Json.put(b, "attachments", Json.copy(Json.array(original, "attachments")));
              Json.put(b, "input", edit.getText().toString());
              persist();
              showChat();
            })
        .show();
  }

  void copy(String value) {
    ((android.content.ClipboardManager) getSystemService(CLIPBOARD_SERVICE))
        .setPrimaryClip(ClipData.newPlainText("Potato", value));
    toast("已复制");
  }

  void share(String value) {
    Intent i = new Intent(Intent.ACTION_SEND);
    i.setType("text/plain");
    i.putExtra(Intent.EXTRA_TEXT, value);
    startActivity(Intent.createChooser(i, "分享"));
  }

  void replyActions(JSONObject m, int index) {
    boolean last = index == Json.array(store.current(), "messages").length() - 1;
    LinearLayout content = column();
    content.setPadding(dp(16), dp(20), dp(16), dp(20));
    AlertDialog d = ParitySheet.show(this, "回复操作", content, last ? 460 : 360);
    if (last) {
      LinearLayout group = ParitySheet.group(this, content);
      ParitySheet.item(
          this,
          group,
          "换模型重新回答",
          m.optString("model") + " · " + ModelPicker.thinkingLabel(m),
          "refresh-cw",
          false,
          () -> {
            d.dismiss();
            settingsScreen.retryModel(index);
          });
    }
    LinearLayout group = ParitySheet.group(this, content);
    ParitySheet.item(
        this,
        group,
        "分享回复",
        null,
        "share",
        false,
        () -> {
          d.dismiss();
          share(m.optString("text"));
        });
    ParitySheet.item(
        this,
        group,
        tts.isSpeaking() ? "停止朗读" : "朗读回复",
        null,
        "volume-2",
        false,
        () -> {
          d.dismiss();
          if (tts.isSpeaking()) tts.stop();
          else tts.speak(m.optString("text"), TextToSpeech.QUEUE_FLUSH, null, Json.id());
        });
    ParitySheet.item(
        this,
        group,
        "选择文字",
        null,
        "type",
        false,
        () -> {
          d.dismiss();
          LinearLayout l = column();
          TextView t = text(m.optString("text"), 17, INK);
          t.setPadding(dp(20), dp(16), dp(20), dp(16));
          t.setTextIsSelectable(true);
          l.addView(t);
          sheet("选择文字", l);
        });
    ParitySheet.item(
        this,
        group,
        "存为工作文稿",
        null,
        "file-text",
        false,
        () -> {
          d.dismiss();
          saveDocument(store.current(), m.optString("text"));
          document(store.current());
        });
  }

  void selectVersion(JSONObject m, int wanted) {
    JSONArray versions = Json.array(m, "versions");
    int current = m.optInt("versionIndex", versions.length());
    if (wanted < 0 || wanted > versions.length() || wanted == current) return;
    JSONArray timeline = new JSONArray();
    JSONObject now = Json.copy(m);
    Json.put(now, "selectedVersion", RecallScreen.messageVersion(m));
    now.remove("versions");
    now.remove("versionIndex");
    for (int i = 0, j = 0; i <= versions.length(); i++)
      timeline.put(i == current ? now : versions.optJSONObject(j++));
    JSONObject chosen = timeline.optJSONObject(wanted);
    Json.put(chosen, "selectedVersion", RecallScreen.messageVersion(chosen));
    JSONArray rest = new JSONArray();
    for (int i = 0; i < timeline.length(); i++)
      if (i != wanted) rest.put(timeline.optJSONObject(i));
    for (String key :
        new String[] {
          "text",
          "reasoning",
          "state",
          "issue",
          "model",
          "effort",
          "thinking",
          "endpoint",
          "searches",
          "recalls",
          "execution",
          "createdAt",
          "selectedVersion"
        }) {
      m.remove(key);
      if (chosen.has(key)) Json.put(m, key, chosen.opt(key));
    }
    Json.put(m, "versions", rest);
    Json.put(m, "versionIndex", wanted);
    persist();
    renderMessages();
  }

  void versions(JSONObject m) {
    JSONArray v = Json.array(m, "versions");
    String[] labels = new String[v.length()];
    for (int i = 0; i < v.length(); i++)
      labels[i] = "版本 " + (i + 1) + " · " + v.optJSONObject(i).optString("model");
    new AlertDialog.Builder(this)
        .setTitle("以前的回复")
        .setItems(
            labels,
            (d, n) -> {
              JSONObject old = Json.copy(m);
              old.remove("versions");
              JSONObject chosen = Json.copy(v.optJSONObject(n));
              try {
                v.put(n, old);
              } catch (Exception ignored) {
              }
              for (String k :
                  new String[] {
                    "text",
                    "reasoning",
                    "state",
                    "issue",
                    "model",
                    "effort",
                    "thinking",
                    "searches",
                    "execution"
                  }) {
                m.remove(k);
                if (chosen.has(k)) Json.put(m, k, chosen.opt(k));
              }
              persist();
              renderMessages();
            })
        .show();
  }

  void sources(JSONObject m) {
    LinearLayout l = column();
    l.setPadding(dp(18), dp(12), dp(18), dp(12));
    JSONArray searches = Json.array(m, "searches");
    for (int i = 0; i < searches.length(); i++) {
      JSONObject s = searches.optJSONObject(i);
      l.addView(text(s.optString("query"), 16, INK));
      JSONArray results = Json.array(s, "results");
      for (int j = 0; j < results.length(); j++) {
        JSONObject r = results.optJSONObject(j);
        l.addView(
            button(r.optString("title", r.optString("url")), () -> openUrl(r.optString("url"))));
        l.addView(
            text(r.optString("content", r.optString("text", r.optString("snippet"))), 13, MUTED));
      }
    }
    sheet("搜索来源", l);
  }

  void openUrl(String url) {
    try {
      Uri uri = Uri.parse(url);
      if (!Arrays.asList("https", "http").contains(uri.getScheme()))
        throw new IOException("链接格式无效。");
      startActivity(new Intent(Intent.ACTION_VIEW, uri));
    } catch (Exception e) {
      error(e);
    }
  }

  void openAttachment(JSONObject f) {
    JSONArray all = Json.array(store.current(), "attachments");
    JSONArray messages = Json.array(store.current(), "messages");
    for (int i = 0; i < messages.length(); i++) {
      JSONArray list = Json.array(messages.optJSONObject(i), "attachments");
      for (int j = 0; j < list.length(); j++)
        if (list.optJSONObject(j).optString("file").equals(f.optString("file"))) all = list;
    }
    new AttachmentPreview(this, all, f).show();
  }

  AlertDialog sheet(String title, LinearLayout content) {
    return ParitySheet.show(this, title, content, 0);
  }

  void expandInput() {
    LinearLayout content = column();
    content.setPadding(dp(16), 0, dp(16), 0);
    EditText edit = field("编辑消息", input.getText().toString());
    edit.setBackgroundColor(CANVAS);
    edit.setTextSize(17);
    edit.setGravity(Gravity.TOP);
    edit.setMinLines(14);
    edit.setSelection(Math.min(Math.max(0, input.getSelectionStart()), edit.length()));
    content.addView(edit);
    AlertDialog dialog = sheet("编辑消息", content);
    if (voice != null) {
      edit.setFocusable(false);
      TextWatcher mirror =
          new TextWatcher() {
            public void beforeTextChanged(CharSequence s, int start, int count, int after) {}

            public void onTextChanged(CharSequence s, int start, int before, int count) {
              edit.setText(s);
              edit.setSelection(edit.length());
            }

            public void afterTextChanged(Editable value) {}
          };
      input.addTextChangedListener(mirror);
      dialog.setOnDismissListener(d -> input.removeTextChangedListener(mirror));
      ParitySheet.actions(
          this,
          dialog,
          "收起",
          dialog::dismiss,
          "完成录音",
          () -> {
            finishVoice(false);
            dialog.dismiss();
          });
    } else {
      watch(
          edit,
          value -> {
            input.setText(value);
            input.setSelection(Math.min(value.length(), Math.max(0, edit.getSelectionStart())));
          });
      ParitySheet.actions(
          this,
          dialog,
          "收起",
          dialog::dismiss,
          "发送",
          () -> {
            if (edit.getText().toString().trim().isEmpty()
                && Json.array(store.current(), "attachments").length() == 0) return;
            dialog.dismiss();
            send();
          });
    }
  }

  void beginNewChat() {
    JSONObject c = store.current();
    if (Json.array(c, "messages").length() == 0
        && c.optString("input").isEmpty()
        && Json.array(c, "attachments").length() == 0
        && !c.optBoolean("deleted")) return;
    store.newChat();
    persist();
    showChat();
  }

  void sidebar() {
    interruptVoice();
    hideKeyboard();
    if (drawer == null || !drawer.isShowing()) {
      drawer = new SidebarView(this);
      drawer.show();
    }
  }

  void moreMenu() {
    PopupMenu menu = new PopupMenu(this, header.getChildAt(header.getChildCount() - 1));
    for (String item : new String[] {"对话", "工作文稿", "添加示例文稿", "记忆与历史", "设置"})
      menu.getMenu().add(item);
    menu.setOnMenuItemClickListener(
        item -> {
          switch (item.getTitle().toString()) {
            case "对话":
              history(false);
              break;
            case "工作文稿":
              document(store.current());
              break;
            case "添加示例文稿":
              ExampleDocument.create(store);
              persist();
              showChat();
              break;
            case "记忆与历史":
              new RecallScreen(this).open();
              break;
            default:
              settingsScreen.open();
          }
          return true;
        });
    menu.show();
  }

  void updateComposer() {
    if (send == null || input == null) return;
    boolean enabled =
        generating != null
            || input.length() > 0
            || Json.array(store.current(), "attachments").length() > 0;
    send.setEnabled(enabled);
    send.setBackground(bg(enabled ? INK : 0xffd1d1cf, 22));
    ((IconButton) send).symbol(generating == null ? "arrow-up" : "square");
    send.setTextColor(Color.WHITE);
    send.setContentDescription(generating == null ? "发送消息" : "停止生成");
    input.post(
        () -> {
          if (input.getLayout() == null) return;
          boolean large =
              input.getLayout().getHeight()
                  > input.getHeight() - input.getPaddingTop() - input.getPaddingBottom() + dp(2);
          overflow.setVisibility(large ? View.VISIBLE : View.GONE);
          characterCount.setText(input.length() + " 字");
        });
  }

  void history(boolean deleted) {
    LinearLayout content = column();
    content.setPadding(dp(16), 0, dp(16), dp(12));
    EditText search = field("搜索标题和消息", "");
    content.addView(search);
    LinearLayout list = column();
    content.addView(list);
    AlertDialog dialog = sheet(deleted ? "最近删除" : "对话", content);
    Runnable render =
        () -> {
          list.removeAllViews();
          ArrayList<JSONObject> chats = new ArrayList<>();
          for (int i = 0; i < store.chats().length(); i++) {
            JSONObject c = store.chats().optJSONObject(i);
            if (c.optBoolean("deleted") == deleted
                && (search.length() == 0
                    || SidebarView.matches(
                        c, search.getText().toString().toLowerCase(Locale.ROOT)))) chats.add(c);
          }
          chats.sort(
              (a, b) -> {
                int pin = Boolean.compare(b.optBoolean("pinned"), a.optBoolean("pinned"));
                return pin != 0
                    ? pin
                    : Long.compare(
                        b.optLong("updated", b.optLong("created")),
                        a.optLong("updated", a.optLong("created")));
              });
          for (JSONObject c : chats) {
            Button b =
                button(
                    (c.optBoolean("pinned") ? "⌁ " : "") + c.optString("title"),
                    () -> {
                      if (deleted) {
                        Json.put(c, "deleted", false);
                        Json.put(store.root, "selected", c.optString("id"));
                        persist();
                        dialog.dismiss();
                        showChat();
                      } else {
                        Json.put(store.root, "selected", c.optString("id"));
                        persist();
                        dialog.dismiss();
                        showChat();
                      }
                    });
            b.setOnLongClickListener(
                v -> {
                  chatOptions(c, dialog);
                  return true;
                });
            list.addView(b);
          }
          if (!deleted)
            list.addView(
                button(
                    "最近删除",
                    () -> {
                      dialog.dismiss();
                      history(true);
                    }));
        };
    watch(search, s -> render.run());
    render.run();
  }

  void chatOptions(JSONObject c, AlertDialog parent) {
    new AlertDialog.Builder(this)
        .setTitle(c.optString("title"))
        .setItems(
            new String[] {
              c.optBoolean("pinned") ? "取消置顶" : "置顶",
              "重命名",
              c.optBoolean("deleted") ? "恢复" : "移到最近删除"
            },
            (d, n) -> {
              if (n == 1) {
                EditText e = field("对话名称", c.optString("title"));
                new AlertDialog.Builder(this)
                    .setTitle("重命名")
                    .setView(e)
                    .setPositiveButton(
                        "保存",
                        (x, w) -> {
                          if (e.length() > 0) Json.put(c, "title", e.getText().toString());
                          persist();
                          parent.dismiss();
                          history(false);
                        })
                    .setNegativeButton("取消", null)
                    .show();
              } else {
                Json.put(
                    c,
                    n == 0 ? "pinned" : "deleted",
                    !(n == 0 ? c.optBoolean("pinned") : c.optBoolean("deleted")));
                if (c.optBoolean("deleted")) {
                  new RecallScreen(this).invalidate(c);
                  new RecallScreen(this).sync(true);
                }
                if (c.optBoolean("deleted") && c == store.current()) store.newChat();
                persist();
                parent.dismiss();
                showChat();
              }
            })
        .show();
  }

  void saveDocument(JSONObject c, String value) {
    Json.put(c, "updated", System.currentTimeMillis());
    String old = c.optString("document");
    if (!old.isEmpty() && !old.equals(value))
      Json.array(c, "documentVersions")
          .put(
              Json.obj(
                  "text",
                  old,
                  "time",
                  System.currentTimeMillis(),
                  "sampleDocument",
                  c.optBoolean("sampleDocument")));
    Json.put(c, "document", value);
    Json.put(c, "sampleDocument", false);
    persist();
  }

  void documents() {
    LinearLayout l = column();
    for (int i = 0; i < store.chats().length(); i++) {
      JSONObject c = store.chats().optJSONObject(i);
      if (!c.optString("document").isEmpty() && !c.optBoolean("deleted"))
        l.addView(button(c.optString("title"), () -> document(c)));
    }
    if (l.getChildCount() == 0) l.addView(text("将任意回复存为工作文稿，即可在这里编辑和分享。", 16, MUTED));
    sheet("工作文稿", l);
  }

  void document(JSONObject c) {
    if (c.optString("document").isEmpty()) {
      message("工作文稿", "将回复存为工作文稿，即可在这里编辑和分享。");
      return;
    }
    new WorkDocumentView(this, c).open();
  }

  void exportText(String name, String value) {
    try {
      File dir = new File(getCacheDir(), "exports");
      dir.mkdirs();
      File f = new File(dir, name.replaceAll("[^\\p{L}\\p{N}._-]", "_"));
      try (FileOutputStream out = new FileOutputStream(f)) {
        out.write(value.getBytes(StandardCharsets.UTF_8));
      }
      shareFile(f, "text/markdown");
    } catch (Exception e) {
      error(e);
    }
  }

  void shareFile(File f, String mime) {
    Intent i =
        new Intent(Intent.ACTION_SEND)
            .setType(mime)
            .putExtra(Intent.EXTRA_STREAM, files.uri(f))
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
    startActivity(Intent.createChooser(i, "分享文件"));
  }

  void startVoice() {
    if (voice != null || generating != null) return;
    if (store.settings().optBoolean("demo")) {
      message("语音输入", "连接支持语音的模型服务后即可使用语音输入。");
      return;
    }
    if (checkSelfPermission(Manifest.permission.RECORD_AUDIO)
        != PackageManager.PERMISSION_GRANTED) {
      requestPermissions(new String[] {Manifest.permission.RECORD_AUDIO}, 42);
      return;
    }
    voiceOriginal = input.getText().toString();
    voiceStart = Math.max(0, input.getSelectionStart());
    voiceEnd = Math.max(voiceStart, input.getSelectionEnd());
    voiceSend = false;
    voiceReady = false;
    voiceFinishing = false;
    voiceTranscript = "";
    String identity = Json.id();
    voiceIdentity = identity;
    String conversation = store.current().optString("id");
    controls.setVisibility(View.GONE);
    voiceRow.removeAllViews();
    input.setFocusable(false);
    input.setHint("正在启动麦克风…");
    hideKeyboard();
    voiceRow.addView(icon("x", "取消本次语音", this::cancelVoice));
    voiceHint = text("准备中", 10, MUTED);
    voiceRow.addView(voiceHint);
    waveform = new VoiceWaveform(this);
    waveform.setContentDescription("正在启动麦克风");
    voiceRow.addView(waveform, new LinearLayout.LayoutParams(0, dp(44), 1));
    waveform.setOnTouchListener(
        new View.OnTouchListener() {
          float x, y;

          public boolean onTouch(View v, MotionEvent event) {
            if (event.getAction() == MotionEvent.ACTION_DOWN) {
              x = event.getX();
              y = event.getY();
              return true;
            }
            if (event.getAction() == MotionEvent.ACTION_UP) {
              float dx = event.getX() - x, dy = event.getY() - y;
              if (dy < -dp(60) && Math.abs(dy) > Math.abs(dx)) finishVoice(true);
              else if (dx < -dp(70) && Math.abs(dx) > Math.abs(dy)) cancelVoice();
              return true;
            }
            return true;
          }
        });
    voiceConfirm = icon("check", "结束录音并发送", () -> finishVoice(true));
    voiceConfirm.setTextColor(Color.WHITE);
    voiceConfirm.setBackground(bg(0xffaaaaaa, 22));
    voiceConfirm.setEnabled(false);
    voiceRow.addView(voiceConfirm);
    input.setOnClickListener(v -> finishVoice(false));
    voice =
        new Voice(
            api,
            new Voice.Listener() {
              boolean current() {
                return identity.equals(voiceIdentity)
                    && conversation.equals(store.current().optString("id"));
              }

              public void ready() {
                if (!current()) return;
                voiceReady = true;
                voiceHint.setText("左滑取消");
                input.setHint("开始说话吧…");
                waveform.setContentDescription("正在听");
              }

              public void limit() {
                if (!current()) return;
                finishVoice(false);
                toast("已录满60秒，文字会保留在输入框。");
              }

              public void text(String value, boolean finished) {
                if (!current()) return;
                if (!value.isEmpty()) {
                  voiceTranscript = value;
                  input.setText(
                      voiceOriginal.substring(0, voiceStart)
                          + value
                          + voiceOriginal.substring(voiceEnd));
                  input.setSelection(Math.min(voiceStart + value.length(), input.length()));
                }
                voiceConfirm.setEnabled(
                    voiceReady && !voiceFinishing && !voiceTranscript.trim().isEmpty());
                voiceConfirm.setBackground(bg(voiceConfirm.isEnabled() ? INK : 0xffaaaaaa, 22));
                if (finished) {
                  boolean shouldSend = voiceSend && foreground && !value.trim().isEmpty(),
                      edit = !voiceSend;
                  releaseVoice();
                  persist();
                  if (shouldSend) send();
                  else if (edit && foreground) {
                    input.requestFocus();
                    ((InputMethodManager) getSystemService(INPUT_METHOD_SERVICE))
                        .showSoftInput(input, InputMethodManager.SHOW_IMPLICIT);
                  }
                  if (value.trim().isEmpty())
                    toast(voiceTranscript.isEmpty() ? "没有听清，请再说一次。" : "未收到完整转写，文字已保留，可修改后发送。");
                }
              }

              public void level(double value) {
                if (current() && voiceReady) waveform.level(value);
              }

              public void error(String issue) {
                if (!current()) return;
                releaseVoice();
                persist();
                message("语音输入", issue + " 未发送，已有文字已保留。");
              }
            });
    try {
      voice.start(store.settings().optString("endpoint"), store.token());
    } catch (Exception e) {
      releaseVoice();
      error(e);
    }
  }

  void finishVoice(boolean send) {
    if (voice == null
        || !voiceReady
        || voiceFinishing
        || (send && voiceTranscript.trim().isEmpty())) return;
    voiceSend = send;
    voiceFinishing = true;
    voiceHint.setText("正在收尾…");
    voiceConfirm.setEnabled(false);
    voice.finish();
  }

  void cancelVoice() {
    if (voice == null) return;
    input.setText(voiceOriginal);
    input.setSelection(Math.min(voiceStart, input.length()), Math.min(voiceEnd, input.length()));
    releaseVoice();
    persist();
  }

  void releaseVoice() {
    voiceIdentity = null;
    if (voice != null) voice.cancel();
    voice = null;
    if (input != null) {
      input.setEnabled(true);
      input.setFocusableInTouchMode(true);
      input.setHint(store.current().optBoolean("documentOpen") ? "想调整哪一部分？" : "问问 Potato");
      input.setOnClickListener(null);
    }
    if (voiceRow != null) voiceRow.removeAllViews();
    if (controls != null) controls.setVisibility(View.VISIBLE);
    updateComposer();
  }

  @Override
  public void onRequestPermissionsResult(int r, String[] p, int[] g) {
    super.onRequestPermissionsResult(r, p, g);
    if (r == 43 && g.length > 0 && g[0] == PackageManager.PERMISSION_GRANTED) {
      remote.startDictation();
      return;
    }
    if (r == 42 && g.length > 0) {
      if (g[0] == PackageManager.PERMISSION_GRANTED) startVoice();
      else message("麦克风权限", "需要麦克风权限才能录音，你仍可使用键盘输入。");
    }
  }

  void interruptVoice() {
    if (voice == null) return;
    releaseVoice();
    persist();
  }

  @Override
  public boolean dispatchTouchEvent(MotionEvent event) {
    if (event.getAction() == MotionEvent.ACTION_DOWN) {
      gestureX = event.getRawX();
      gestureY = event.getRawY();
      drawerGesture =
          !remote.visible
              && voice == null
              && !inside(composer, gestureX, gestureY)
              && !horizontalAt(root, gestureX, gestureY);
    }
    if (drawerGesture && event.getAction() == MotionEvent.ACTION_MOVE) {
      float dx = event.getRawX() - gestureX, dy = event.getRawY() - gestureY;
      if (Math.abs(dy) > dp(18) && Math.abs(dy) > Math.abs(dx)) drawerGesture = false;
      else if (dx > dp(80) && dx > Math.abs(dy) * 1.5f) {
        drawerGesture = false;
        MotionEvent cancel = MotionEvent.obtain(event);
        cancel.setAction(MotionEvent.ACTION_CANCEL);
        super.dispatchTouchEvent(cancel);
        cancel.recycle();
        sidebar();
        return true;
      }
    }
    return super.dispatchTouchEvent(event);
  }

  boolean inside(View v, float x, float y) {
    if (v == null || v.getVisibility() != View.VISIBLE) return false;
    int[] pos = new int[2];
    v.getLocationOnScreen(pos);
    return x >= pos[0] && x < pos[0] + v.getWidth() && y >= pos[1] && y < pos[1] + v.getHeight();
  }

  boolean horizontalAt(View v, float x, float y) {
    if (!inside(v, x, y)) return false;
    if (v instanceof HorizontalScrollView || v instanceof EditText) return true;
    if (v instanceof ViewGroup g)
      for (int i = 0; i < g.getChildCount(); i++)
        if (horizontalAt(g.getChildAt(i), x, y)) return true;
    return false;
  }

  void sandbox(JSONObject message) {
    Matcher matcher =
        Pattern.compile("```(?:python|py)\\s*\\n([\\s\\S]*?)```", Pattern.CASE_INSENSITIVE)
            .matcher(message.optString("text"));
    if (!matcher.find()) {
      message("云端计算", "回复中没有 Python 代码块。");
      return;
    }
    sandbox(message, matcher.group(1));
  }

  void sandbox(JSONObject message, String code) {
    LinearLayout l = column();
    l.setPadding(dp(14), 0, dp(14), 0);
    EditText editor = field("Python 代码", code);
    editor.setTypeface(Typeface.MONOSPACE);
    l.addView(editor);
    JSONArray selected = new JSONArray();
    JSONArray attachments = Json.copy(Json.array(store.current(), "attachments"));
    JSONArray chatMessages = Json.array(store.current(), "messages");
    for (int n = 0; n < chatMessages.length(); n++) {
      JSONObject item = chatMessages.optJSONObject(n);
      if (item == message) break;
      JSONArray source = Json.array(item, "attachments");
      for (int j = 0; j < source.length(); j++) attachments.put(source.optJSONObject(j));
    }
    for (int i = 0; i < attachments.length(); i++) {
      JSONObject f = attachments.optJSONObject(i);
      CheckBox box = new CheckBox(this);
      box.setText(f.optString("name"));
      box.setOnCheckedChangeListener(
          (b, on) -> {
            if (on) selected.put(f);
            else
              for (int j = selected.length() - 1; j >= 0; j--)
                if (selected.optJSONObject(j) == f) selected.remove(j);
          });
      l.addView(box);
    }
    TextView output = text("将代码与勾选的附件提交到云端运行。", 14, MUTED);
    l.addView(output);
    Button run = button("运行代码", () -> {});
    l.addView(run);
    run.setOnClickListener(
        v -> {
          try {
            String
                endpoint =
                    Api.servicePath(store.settings().optString("endpoint"), "/v1/sandbox/run"),
                token = store.token(),
                source = editor.getText().toString();
            if (source.length() > 32000) throw new IOException("代码最多 32000 字符。");
            run.setEnabled(false);
            output.setText("正在运行…");
            worker.execute(
                () -> {
                  try {
                    JSONArray inputs = new JSONArray();
                    int total = 0;
                    for (int i = 0; i < selected.length(); i++) {
                      JSONObject f = selected.optJSONObject(i);
                      byte[] bytes = Attachments.read(new FileInputStream(files.file(f)), 2000000);
                      total += bytes.length;
                      if (total > 2000000) throw new IOException("云端计算输入最多 2 MB。");
                      inputs.put(
                          Json.obj(
                              "name",
                              "input-"
                                  + (i + 1)
                                  + f.optString("file")
                                      .substring(f.optString("file").lastIndexOf('.')),
                              "base64",
                              android.util.Base64.encodeToString(
                                  bytes, android.util.Base64.NO_WRAP)));
                    }
                    JSONObject result =
                        api.json(endpoint, token, Json.obj("code", source, "files", inputs));
                    main.post(
                        () -> {
                          Json.put(message, "execution", result);
                          persist();
                          run.setEnabled(true);
                          renderExecution(result, output, l);
                          renderMessages();
                        });
                  } catch (Exception e) {
                    main.post(
                        () -> {
                          run.setEnabled(true);
                          output.setText(e.getMessage());
                        });
                  }
                });
          } catch (Exception e) {
            error(e);
          }
        });
    JSONObject prior = message.optJSONObject("execution");
    if (prior != null) renderExecution(prior, output, l);
    sheet("云端计算", l);
  }

  void renderExecution(JSONObject result, TextView output, LinearLayout l) {
    output.setText(
        result.optString("stdout")
            + "\n"
            + result.optString("stderr")
            + "\n"
            + result.optString("text")
            + (result.isNull("error") ? "" : result.optString("error")));
    JSONArray artifacts = Json.array(result, "artifacts");
    for (int i = 0; i < artifacts.length(); i++) {
      JSONObject artifact = artifacts.optJSONObject(i);
      l.addView(
          button(
              "分享 " + artifact.optString("name"),
              () -> {
                try {
                  File dir = new File(getCacheDir(), "exports");
                  dir.mkdirs();
                  File f =
                      new File(
                          dir,
                          Json.id()
                              + "-"
                              + artifact.optString("name").replaceAll("[^\\p{L}\\p{N}._-]", "_"));
                  try (FileOutputStream out = new FileOutputStream(f)) {
                    out.write(
                        android.util.Base64.decode(
                            artifact.optString("base64"), android.util.Base64.DEFAULT));
                  }
                  shareFile(f, artifact.optString("mime", "application/octet-stream"));
                } catch (Exception e) {
                  error(e);
                }
              }));
    }
  }

  @Override
  public void onBackPressed() {
    if (remote.visible) {
      remote.back();
      return;
    }
    if (voice != null) {
      interruptVoice();
      return;
    }
    if (store.current().optBoolean("documentOpen")) {
      Json.put(store.current(), "documentOpen", false);
      persist();
      showChat();
      return;
    }
    super.onBackPressed();
  }
}
