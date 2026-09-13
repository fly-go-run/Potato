package top.recodex.potato;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.drawable.Drawable;
import android.widget.Button;

/** Actual Lucide vector assets shared with the maintained desktop client. */
final class IconButton extends Button {
  private Drawable glyph;
  private int tint = MainActivity.INK;

  IconButton(Context context, String name) {
    super(context);
    setText("");
    setPadding(0, 0, 0, 0);
    setMinWidth(0);
    setMinimumWidth(0);
    setMinHeight(0);
    setMinimumHeight(0);
    setBackgroundColor(Color.TRANSPARENT);
    setStateListAnimator(null);
    setElevation(0);
    symbol(name);
  }

  void symbol(String name) {
    name =
        switch (name) {
          case "☰" -> "menu";
          case "＋" -> "plus";
          case "↑" -> "arrow-up";
          case "■" -> "square";
          case "♩" -> "mic";
          case "↗" -> "expand";
          case "‹", "←" -> "chevron-left";
          case "×" -> "x";
          default -> name;
        };
    int id =
        getResources()
            .getIdentifier(
                "lucide_" + name.replace('-', '_'), "drawable", getContext().getPackageName());
    glyph = id == 0 ? null : getContext().getDrawable(id).mutate();
    invalidate();
  }

  @Override
  public void setTextColor(int value) {
    super.setTextColor(value);
    tint = value;
    invalidate();
  }

  @Override
  protected void onDraw(Canvas canvas) {
    super.onDraw(canvas);
    if (glyph != null) {
      int size = Math.round(20 * getResources().getDisplayMetrics().density);
      int x = (getWidth() - size) / 2, y = (getHeight() - size) / 2;
      glyph.setBounds(x, y, x + size, y + size);
      glyph.setTint(tint);
      glyph.draw(canvas);
    }
  }
}
