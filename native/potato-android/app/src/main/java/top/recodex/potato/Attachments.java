package top.recodex.potato;

import android.content.*;
import android.database.Cursor;
import android.graphics.*;
import android.net.Uri;
import android.provider.OpenableColumns;
import android.util.Base64;
import androidx.core.content.FileProvider;
import androidx.exifinterface.media.ExifInterface;
import com.tom_roush.pdfbox.pdmodel.PDDocument;
import com.tom_roush.pdfbox.text.PDFTextStripper;
import java.io.*;
import java.nio.*;
import java.nio.charset.*;
import org.json.*;

final class Attachments {
  final Context context;

  Attachments(Context c) {
    context = c;
    com.tom_roush.pdfbox.android.PDFBoxResourceLoader.init(c);
  }

  File file(JSONObject a) throws IOException {
    String name = a.optString("file");
    if (!name.matches("[a-zA-Z0-9_.-]+")) throw new IOException("附件路径无效。");
    return new File(new File(context.getFilesDir(), "attachments"), name);
  }

  static byte[] read(InputStream in, int max) throws IOException {
    if (in == null) throw new IOException("无法打开文件。");
    try (in;
        ByteArrayOutputStream out = new ByteArrayOutputStream()) {
      byte[] buf = new byte[8192];
      int n;
      while ((n = in.read(buf)) != -1) {
        if (out.size() + n > max) throw new IOException("单文件最多 10 MB，请缩小后重试。");
        out.write(buf, 0, n);
      }
      return out.toByteArray();
    }
  }

  JSONObject importUri(Uri uri) throws Exception {
    String name = "附件";
    try (Cursor c =
        context
            .getContentResolver()
            .query(uri, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
      if (c != null && c.moveToFirst()) name = c.getString(0);
    }
    String type = context.getContentResolver().getType(uri);
    if (type == null) {
      String extension = android.webkit.MimeTypeMap.getFileExtensionFromUrl(uri.toString());
      type =
          android.webkit.MimeTypeMap.getSingleton()
              .getMimeTypeFromExtension(extension.toLowerCase(java.util.Locale.ROOT));
    }
    if (type == null) type = "application/octet-stream";
    byte[] data = read(context.getContentResolver().openInputStream(uri), 10 * 1024 * 1024);
    String extracted = null;
    if (type.startsWith("image/")) {
      BitmapFactory.Options options = new BitmapFactory.Options();
      options.inJustDecodeBounds = true;
      BitmapFactory.decodeByteArray(data, 0, data.length, options);
      options.inSampleSize = 1;
      while (Math.max(options.outWidth, options.outHeight) / options.inSampleSize > 2400)
        options.inSampleSize *= 2;
      options.inJustDecodeBounds = false;
      Bitmap b = BitmapFactory.decodeByteArray(data, 0, data.length, options);
      if (b == null) throw new IOException("无法读取此图片，请选择 JPEG 或 PNG。");
      ExifInterface exif = new ExifInterface(new ByteArrayInputStream(data));
      Matrix matrix = new Matrix();
      matrix.postRotate(exif.getRotationDegrees());
      if (exif.isFlipped()) matrix.postScale(-1, 1);
      Bitmap oriented = Bitmap.createBitmap(b, 0, 0, b.getWidth(), b.getHeight(), matrix, true);
      if (oriented != b) b.recycle();
      b = oriented;
      float scale = Math.min(1f, 1600f / Math.max(b.getWidth(), b.getHeight()));
      Bitmap smaller =
          Bitmap.createScaledBitmap(
              b,
              Math.max(1, (int) (b.getWidth() * scale)),
              Math.max(1, (int) (b.getHeight() * scale)),
              true);
      if (smaller != b) b.recycle();
      b = smaller;
      for (int q = 85; ; q -= 10) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        b.compress(Bitmap.CompressFormat.JPEG, q, out);
        data = out.toByteArray();
        if (data.length <= 600_000) break;
        if (q <= 35) {
          Bitmap next =
              Bitmap.createScaledBitmap(
                  b, Math.max(1, b.getWidth() * 3 / 4), Math.max(1, b.getHeight() * 3 / 4), true);
          b.recycle();
          b = next;
          q = 85;
        }
      }
      b.recycle();
      type = "image/jpeg";
    } else if (type.equals("application/pdf")
        || name.toLowerCase(java.util.Locale.ROOT).endsWith(".pdf")) {
      try (PDDocument pdf = PDDocument.load(data)) {
        PDFTextStripper stripper = new PDFTextStripper();
        stripper.setEndPage(200);
        extracted = stripper.getText(pdf);
        if (extracted.trim().isEmpty()) throw new IOException("这个 PDF 没有可提取文字，请使用图片或带文字的 PDF。");
        if (pdf.getNumberOfPages() > 200) throw new IOException("PDF 超过 200 页，请拆分后导入。");
      }
      type = "application/pdf";
    } else {
      try {
        extracted =
            StandardCharsets.UTF_8
                .newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(data))
                .toString();
      } catch (CharacterCodingException e) {
        throw new IOException("聊天支持图片、文字 PDF 和 UTF-8 文本文件。");
      }
      if (extracted.indexOf('\0') >= 0) throw new IOException("此文件不是文本，请先转换为文本或 PDF。");
    }
    if (extracted != null && extracted.length() > 200_000) throw new IOException("附件文字过长，请拆分文件。");
    String filename =
        Json.id()
            + (type.equals("image/jpeg")
                ? ".jpg"
                : type.equals("application/pdf") ? ".pdf" : ".txt");
    JSONObject value = Json.obj("name", name, "type", type, "file", filename, "size", data.length);
    if (extracted != null) Json.put(value, "text", extracted);
    File f = file(value);
    f.getParentFile().mkdirs();
    try (FileOutputStream out = new FileOutputStream(f)) {
      out.write(data);
    }
    return value;
  }

  JSONArray wire(JSONArray messages) throws Exception {
    int count = 0;
    for (int i = 0; i < messages.length(); i++) {
      JSONObject m = messages.optJSONObject(i);
      JSONArray a = Json.array(m, "attachments");
      for (int j = 0; j < a.length(); j++)
        if (a.optJSONObject(j).optString("type").startsWith("image/")) count++;
    }
    int budget = 2_400_000 / Math.max(4, count);
    JSONArray wire = new JSONArray();
    for (int i = 0; i < messages.length(); i++) {
      JSONObject m = messages.optJSONObject(i);
      String text = m.optString("text");
      if (m.optString("state").equals("failed")) continue;
      JSONArray parts = new JSONArray(), a = Json.array(m, "attachments");
      for (int j = 0; j < a.length(); j++) {
        JSONObject f = a.optJSONObject(j);
        if (f.optString("type").startsWith("image/")) {
          byte[] bytes = read(new FileInputStream(file(f)), 10 * 1024 * 1024);
          if (bytes.length > budget) {
            Bitmap b = BitmapFactory.decodeByteArray(bytes, 0, bytes.length);
            while (bytes.length > budget) {
              Bitmap next =
                  Bitmap.createScaledBitmap(
                      b,
                      Math.max(1, b.getWidth() * 3 / 4),
                      Math.max(1, b.getHeight() * 3 / 4),
                      true);
              if (next != b) b.recycle();
              b = next;
              ByteArrayOutputStream out = new ByteArrayOutputStream();
              b.compress(Bitmap.CompressFormat.JPEG, 70, out);
              bytes = out.toByteArray();
            }
            b.recycle();
          }
          parts.put(
              Json.obj(
                  "type",
                  "image_url",
                  "image_url",
                  Json.obj(
                      "url",
                      "data:image/jpeg;base64," + Base64.encodeToString(bytes, Base64.NO_WRAP))));
        } else
          text +=
              "\n\n<attachment name=\""
                  + f.optString("name")
                  + "\">\n"
                  + f.optString("text")
                  + "\n</attachment>";
      }
      if (text.isEmpty() && parts.length() == 0) continue;
      JSONArray content = new JSONArray();
      content.put(Json.obj("type", "text", "text", text));
      for (int j = 0; j < parts.length(); j++) content.put(parts.get(j));
      wire.put(
          Json.obj("role", m.optString("role"), "content", parts.length() == 0 ? text : content));
    }
    return wire;
  }

  Uri uri(File file) {
    return FileProvider.getUriForFile(context, context.getPackageName() + ".files", file);
  }
}
