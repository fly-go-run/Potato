package top.recodex.potato;

import android.app.Dialog;
import android.graphics.*;
import android.graphics.drawable.ColorDrawable;
import android.net.Uri;
import android.view.*;
import android.widget.*;
import java.util.*;
import org.json.*;

final class AttachmentPreview extends Dialog {
  final MainActivity a;
  final ArrayList<JSONObject> items = new ArrayList<>();
  int index, pdfPage;
  android.graphics.pdf.PdfRenderer pdf;
  android.os.ParcelFileDescriptor pdfFile;
  Bitmap pdfBitmap;
  ImageView pdfImage;
  TextView pdfPosition;
  LinearLayout root;
  TextView title;
  FrameLayout stage;
  float x;

  AttachmentPreview(MainActivity a, JSONArray list, JSONObject selected) {
    super(a);
    this.a = a;
    for (int i = 0; i < list.length(); i++) {
      JSONObject f = list.optJSONObject(i);
      if (f.optString("type").startsWith("image/")) {
        if (f.optString("file").equals(selected.optString("file"))) index = items.size();
        items.add(f);
      }
    }
    if (items.isEmpty() || !selected.optString("type").startsWith("image/")) {
      items.clear();
      items.add(selected);
      index = 0;
    }
  }

  @Override
  public void show() {
    root = a.column();
    root.setBackgroundColor(MainActivity.CANVAS);
    root.setPadding(0, a.dp(40), 0, a.dp(28));
    LinearLayout bar = a.row();
    bar.setPadding(a.dp(16), 0, a.dp(16), a.dp(12));
    bar.addView(a.icon("x", "关闭预览", this::dismiss));
    title = a.text("", 17, MainActivity.INK);
    title.setSingleLine(true);
    title.setEllipsize(android.text.TextUtils.TruncateAt.END);
    title.setGravity(Gravity.CENTER);
    bar.addView(title, new LinearLayout.LayoutParams(0, -2, 1));
    bar.addView(
        a.icon(
            "share",
            "分享附件",
            () -> {
              try {
                JSONObject f = items.get(index);
                a.shareFile(a.files.file(f), f.optString("type"));
              } catch (Exception e) {
                a.error(e);
              }
            }));
    root.addView(bar);
    stage = new FrameLayout(a);
    root.addView(stage, new LinearLayout.LayoutParams(-1, 0, 1));
    if (items.size() > 1) {
      LinearLayout controls = a.row();
      controls.setGravity(Gravity.CENTER);
      controls.addView(a.icon("chevron-left", "上一张", () -> change(-1)));
      controls.addView(a.icon("chevron-right", "下一张", () -> change(1)));
      root.addView(controls);
    }
    setContentView(root);
    super.show();
    getWindow().setBackgroundDrawable(new ColorDrawable(MainActivity.CANVAS));
    getWindow().setLayout(-1, -1);
    getWindow()
        .getDecorView()
        .setSystemUiVisibility(
            View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
    render();
  }

  void change(int amount) {
    int next = index + amount;
    if (next < 0 || next >= items.size()) return;
    index = next;
    render();
  }

  void render() {
    stage.removeAllViews();
    JSONObject f = items.get(index);
    title.setText(items.size() > 1 ? (index + 1) + " / " + items.size() : f.optString("name"));
    try {
      if (f.optString("type").startsWith("image/")) {
        ZoomImage image = new ZoomImage(a);
        image.setImageURI(Uri.fromFile(a.files.file(f)));
        image.setContentDescription(f.optString("name"));
        stage.addView(image, new FrameLayout.LayoutParams(-1, -1));
      } else if (f.optString("type").equals("application/pdf")) {
        pdfFile =
            android.os.ParcelFileDescriptor.open(
                a.files.file(f), android.os.ParcelFileDescriptor.MODE_READ_ONLY);
        pdf = new android.graphics.pdf.PdfRenderer(pdfFile);
        LinearLayout document = a.column();
        LinearLayout nav = a.row();
        nav.addView(
            a.icon(
                "chevron-left",
                "上一页",
                () -> {
                  if (pdfPage > 0) {
                    pdfPage--;
                    renderPdf();
                  }
                }));
        pdfPosition = a.text("", 14, MainActivity.MUTED);
        nav.addView(pdfPosition, new LinearLayout.LayoutParams(0, -2, 1));
        nav.addView(
            a.icon(
                "chevron-right",
                "下一页",
                () -> {
                  if (pdfPage + 1 < pdf.getPageCount()) {
                    pdfPage++;
                    renderPdf();
                  }
                }));
        document.addView(nav);
        pdfImage = new ImageView(a);
        pdfImage.setScaleType(ImageView.ScaleType.FIT_CENTER);
        document.addView(pdfImage, new LinearLayout.LayoutParams(-1, 0, 1));
        stage.addView(document, new FrameLayout.LayoutParams(-1, -1));
        renderPdf();
      } else {
        LinearLayout content = a.column();
        TextView name = a.text(f.optString("name"), 20, MainActivity.INK);
        content.addView(name);
        TextView text = a.text(f.optString("text", "此文件没有可预览的文字。"), 16, MainActivity.INK);
        text.setTextIsSelectable(true);
        content.addView(text);
        content.setPadding(a.dp(20), a.dp(16), a.dp(20), a.dp(20));
        ScrollView scroll = new ScrollView(a);
        scroll.addView(content);
        stage.addView(scroll);
      }
    } catch (Exception e) {
      stage.addView(a.text("附件无法读取：" + e.getMessage(), 16, MainActivity.MUTED));
    }
  }

  void renderPdf() {
    try (android.graphics.pdf.PdfRenderer.Page page = pdf.openPage(pdfPage)) {
      float scale = Math.min(1200f / page.getWidth(), 1800f / page.getHeight());
      Bitmap image =
          Bitmap.createBitmap(
              Math.max(1, Math.round(page.getWidth() * scale)),
              Math.max(1, Math.round(page.getHeight() * scale)),
              Bitmap.Config.ARGB_8888);
      image.eraseColor(Color.WHITE);
      page.render(image, null, null, android.graphics.pdf.PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY);
      pdfImage.setImageBitmap(image);
      if (pdfBitmap != null) pdfBitmap.recycle();
      pdfBitmap = image;
      pdfPosition.setText((pdfPage + 1) + " / " + pdf.getPageCount());
    } catch (Exception e) {
      a.error(e);
    }
  }

  @Override
  public void dismiss() {
    if (pdfImage != null) pdfImage.setImageDrawable(null);
    if (pdfBitmap != null) {
      pdfBitmap.recycle();
      pdfBitmap = null;
    }
    if (pdf != null) {
      pdf.close();
      pdf = null;
    }
    if (pdfFile != null) {
      try {
        pdfFile.close();
      } catch (Exception ignored) {
      }
      pdfFile = null;
    }
    super.dismiss();
  }

  final class ZoomImage extends ImageView {
    float scale = 1, lastX, lastY;
    final ScaleGestureDetector pinch;
    final GestureDetector taps;

    ZoomImage(MainActivity a) {
      super(a);
      setScaleType(ScaleType.FIT_CENTER);
      pinch =
          new ScaleGestureDetector(
              a,
              new ScaleGestureDetector.SimpleOnScaleGestureListener() {
                @Override
                public boolean onScale(ScaleGestureDetector d) {
                  scale = Math.min(5, Math.max(1, scale * d.getScaleFactor()));
                  setScaleX(scale);
                  setScaleY(scale);
                  return true;
                }
              });
      taps =
          new GestureDetector(
              a,
              new GestureDetector.SimpleOnGestureListener() {
                @Override
                public boolean onDoubleTap(MotionEvent e) {
                  scale = scale > 1 ? 1 : 2;
                  setScaleX(scale);
                  setScaleY(scale);
                  setTranslationX(0);
                  setTranslationY(0);
                  return true;
                }
              });
    }

    @Override
    public boolean onTouchEvent(MotionEvent e) {
      pinch.onTouchEvent(e);
      taps.onTouchEvent(e);
      if (e.getAction() == MotionEvent.ACTION_DOWN) {
        x = e.getX();
        lastX = e.getRawX();
        lastY = e.getRawY();
      } else if (e.getAction() == MotionEvent.ACTION_MOVE && scale > 1 && !pinch.isInProgress()) {
        setTranslationX(
            Math.max(
                -getWidth() * (scale - 1) / 2,
                Math.min(getWidth() * (scale - 1) / 2, getTranslationX() + e.getRawX() - lastX)));
        setTranslationY(
            Math.max(
                -getHeight() * (scale - 1) / 2,
                Math.min(getHeight() * (scale - 1) / 2, getTranslationY() + e.getRawY() - lastY)));
        lastX = e.getRawX();
        lastY = e.getRawY();
      } else if (e.getAction() == MotionEvent.ACTION_UP
          && scale == 1
          && Math.abs(e.getX() - x) > a.dp(60)) {
        change(e.getX() < x ? 1 : -1);
      }
      return true;
    }
  }
}
