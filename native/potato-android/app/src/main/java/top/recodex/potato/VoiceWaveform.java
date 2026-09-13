package top.recodex.potato;

import android.content.Context;
import android.graphics.*;
import android.view.View;

/** Live microphone amplitude history, matching the compact iOS 21-sample meter. */
final class VoiceWaveform extends View {
  final float[] levels = new float[21];
  final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);

  VoiceWaveform(Context c) {
    super(c);
  }

  void level(double value) {
    System.arraycopy(levels, 1, levels, 0, 20);
    levels[20] = (float) Math.max(0, Math.min(1, value));
    invalidate();
  }

  @Override
  protected void onDraw(Canvas canvas) {
    float density = getResources().getDisplayMetrics().density;
    for (int i = 0; i < getWidth() / (4.5f * density); i++) {
      paint.setColor(i < 21 ? MainActivity.INK : 0xffdededb);
      float h = (3 + (i < 21 ? levels[i] * 11 : 0)) * density, x = i * 4.5f * density;
      canvas.drawRoundRect(
          x,
          (getHeight() - h) / 2,
          x + 2 * density,
          (getHeight() + h) / 2,
          density,
          density,
          paint);
    }
  }
}
