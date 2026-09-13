package top.recodex.potato;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.widget.Button;

/** Monochrome microphone with the same visual weight as the other composer controls. */
final class MicrophoneButton extends Button {
  private final Paint stroke = new Paint(Paint.ANTI_ALIAS_FLAG);

  MicrophoneButton(Context context) {
    super(context);
  }

  @Override
  protected void onDraw(Canvas canvas) {
    float size = 23 * getResources().getDisplayMetrics().density;
    canvas.save();
    canvas.translate((getWidth() - size) / 2, (getHeight() - size) / 2);
    canvas.scale(size / 24, size / 24);
    stroke.setColor(MainActivity.INK);
    stroke.setStyle(Paint.Style.STROKE);
    stroke.setStrokeWidth(1.8f);
    stroke.setStrokeCap(Paint.Cap.ROUND);
    canvas.drawRoundRect(9, 2, 15, 15, 3, 3, stroke);
    canvas.drawArc(5, 7, 19, 20, 0, 180, false, stroke);
    canvas.drawLine(5, 11, 5, 13, stroke);
    canvas.drawLine(19, 11, 19, 13, stroke);
    canvas.drawLine(12, 20, 12, 23, stroke);
    canvas.restore();
  }
}
