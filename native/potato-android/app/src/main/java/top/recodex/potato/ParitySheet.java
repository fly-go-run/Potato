package top.recodex.potato;

import android.app.AlertDialog;
import android.graphics.Color;
import android.graphics.drawable.ColorDrawable;
import android.view.*;
import android.widget.*;

final class ParitySheet {
  static AlertDialog show(MainActivity a, String title, LinearLayout content, int height) {
    LinearLayout panel = a.column();
    panel.setBackground(a.bg(0xfff7f7f7, 32));
    panel.setClipToOutline(true);
    View handle = new View(a);
    handle.setBackground(a.bg(0xffcececb, 3));
    LinearLayout.LayoutParams hp = new LinearLayout.LayoutParams(a.dp(36), a.dp(5));
    hp.gravity = Gravity.CENTER_HORIZONTAL;
    hp.topMargin = a.dp(8);
    hp.bottomMargin = a.dp(5);
    panel.addView(handle, hp);
    LinearLayout bar = a.row();
    bar.setTag("parity-header");
    bar.setPadding(a.dp(16), 0, a.dp(16), a.dp(8));
    AlertDialog d = new AlertDialog.Builder(a).create();
    Button close = a.icon("x", "完成", d::dismiss);
    close.setBackground(a.bg(Color.WHITE, 22));
    bar.addView(close);
    TextView label = a.text(title, 17, MainActivity.INK);
    label.setTypeface(null, android.graphics.Typeface.BOLD);
    label.setGravity(Gravity.CENTER);
    bar.addView(label, new LinearLayout.LayoutParams(0, a.dp(44), 1));
    bar.addView(new View(a), new LinearLayout.LayoutParams(a.dp(44), 1));
    panel.addView(bar);
    ScrollView scroll = new ScrollView(a);
    scroll.setVerticalScrollBarEnabled(false);
    scroll.setFillViewport(false);
    scroll.addView(content);
    panel.addView(scroll, new LinearLayout.LayoutParams(-1, 0, 1));
    d.setView(panel, 0, 0, 0, 0);
    d.show();
    Window w = d.getWindow();
    w.setBackgroundDrawable(new ColorDrawable(Color.TRANSPARENT));
    w.setGravity(Gravity.BOTTOM);
    w.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
    w.setDimAmount(.18f);
    int available = a.getResources().getDisplayMetrics().heightPixels;
    w.getDecorView()
        .setSystemUiVisibility(
            View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
    w.setLayout(
        height > 0 ? a.getResources().getDisplayMetrics().widthPixels - a.dp(16) : -1,
        height > 0 ? Math.min(a.dp(height), available - a.dp(48)) : available - a.dp(60));
    panel.setPadding(0, 0, 0, a.dp(16));
    return d;
  }

  static void actions(
      MainActivity a, AlertDialog d, String left, Runnable cancel, String right, Runnable confirm) {
    LinearLayout bar = d.getWindow().getDecorView().findViewWithTag("parity-header");
    if (bar == null) return;
    TextView label = (TextView) bar.getChildAt(1);
    bar.removeView(label);
    bar.removeAllViews();
    Button back = a.button(left, cancel);
    back.setBackground(a.bg(Color.WHITE, 22));
    bar.addView(back);
    bar.addView(label, new LinearLayout.LayoutParams(0, a.dp(44), 1));
    Button done = a.button(right, confirm);
    done.setTypeface(null, android.graphics.Typeface.BOLD);
    done.setBackground(a.bg(Color.WHITE, 22));
    bar.addView(done);
  }

  static LinearLayout group(MainActivity a, LinearLayout parent) {
    LinearLayout group = a.column();
    group.setBackground(a.bg(Color.WHITE, 28));
    group.setClipToOutline(true);
    LinearLayout.LayoutParams p = new LinearLayout.LayoutParams(-1, -2);
    p.setMargins(0, 0, 0, a.dp(16));
    parent.addView(group, p);
    return group;
  }

  static void item(
      MainActivity a,
      LinearLayout group,
      String title,
      String detail,
      String icon,
      boolean selected,
      Runnable action) {
    LinearLayout row = a.row();
    row.setPadding(a.dp(16), a.dp(5), a.dp(16), a.dp(5));
    row.setMinimumHeight(a.dp(detail != null && !detail.isEmpty() ? 74 : 60));
    boolean disclosure = "chevron-right".equals(icon);
    if (icon != null && !disclosure) {
      Button image = a.icon(icon, title, action);
      row.addView(image);
    }
    LinearLayout labels = a.column();
    TextView name = a.text(title, 17, MainActivity.INK);
    labels.addView(name);
    if (!disclosure && detail != null && !detail.isEmpty())
      labels.addView(a.text(detail, 13, MainActivity.MUTED));
    row.addView(labels, new LinearLayout.LayoutParams(0, -2, 1));
    if (disclosure) {
      if (detail != null) row.addView(a.text(detail, 16, MainActivity.MUTED));
      Button chevron = a.icon("chevron-right", title, action);
      chevron.setTextColor(0xffc6c6c4);
      row.addView(chevron);
    }
    if (group.getChildCount() > 0) {
      View divider = new View(a);
      divider.setBackgroundColor(MainActivity.LINE);
      LinearLayout.LayoutParams dp = new LinearLayout.LayoutParams(-1, a.dp(.5f));
      dp.setMargins(a.dp(16), 0, a.dp(16), 0);
      group.addView(divider, dp);
    }
    if (selected) {
      Button check = a.icon("check", "已选择", action);
      check.setTextColor(0xff007aff);
      row.addView(check);
    }
    row.setContentDescription(title);
    row.setFocusable(true);
    row.setOnClickListener(v -> action.run());
    group.addView(row);
  }
}
