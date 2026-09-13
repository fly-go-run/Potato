package top.recodex.potato;

import android.content.Context;
import android.security.keystore.*;
import android.util.AtomicFile;
import android.util.Base64;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.security.KeyStore;
import javax.crypto.*;
import javax.crypto.spec.GCMParameterSpec;
import org.json.*;

final class Store {
  static final String RELAY = "https://potato-remote.recodex.top";
  final Context context;
  final AtomicFile file;
  JSONObject root;
  boolean corrupt;
  String loadIssue;

  Store(Context c) {
    context = c;
    file = new AtomicFile(new File(c.getFilesDir(), "workspace.json"));
    try {
      root = new JSONObject(new String(file.readFully(), StandardCharsets.UTF_8));
      validate(root);
    } catch (FileNotFoundException e) {
      root = Json.obj();
    } catch (Exception e) {
      root = Json.obj();
      corrupt = true;
      loadIssue = "本机记录无法读取，原文件已保留。请导出备份后再恢复使用。";
    }
    if (root.optJSONObject("settings") == null)
      Json.put(
          root,
          "settings",
          Json.obj(
              "demo",
              true,
              "endpoint",
              "",
              "model",
              "",
              "prompt",
              "你是 Potato，一位细心、可靠的中文助手。回答简洁自然，按需要使用 Markdown。"));
    if (chats().length() == 0) ExampleDocument.create(this);
    for (int i = 0; i < chats().length(); i++)
      for (int j = 0; j < Json.array(chats().optJSONObject(i), "messages").length(); j++) {
        JSONObject m = Json.array(chats().optJSONObject(i), "messages").optJSONObject(j);
        if (m.optString("state").equals("streaming")) {
          Json.put(m, "state", "interrupted");
          Json.put(m, "issue", "上次生成被中断，已保留收到的内容。");
        }
      }
  }

  private static void validate(JSONObject data) throws JSONException {
    if (data.has("settings") && data.optJSONObject("settings") == null)
      throw new JSONException("Invalid settings");
    if (data.has("chats") && data.optJSONArray("chats") == null)
      throw new JSONException("Invalid conversations");
    JSONArray chats = data.optJSONArray("chats");
    if (chats == null) return;
    for (int i = 0; i < chats.length(); i++) {
      JSONObject chat = chats.getJSONObject(i);
      if (chat.optString("id").isEmpty()) throw new JSONException("Missing conversation identity");
      for (String key : new String[] {"messages", "attachments", "documentVersions"}) {
        if (chat.has(key) && chat.optJSONArray(key) == null)
          throw new JSONException("Invalid conversation data");
        JSONArray entries = chat.optJSONArray(key);
        if (entries != null) for (int j = 0; j < entries.length(); j++) entries.getJSONObject(j);
      }
    }
  }

  JSONObject settings() {
    return Json.object(root, "settings");
  }

  JSONArray chats() {
    return Json.array(root, "chats");
  }

  JSONObject current() {
    for (int i = 0; i < chats().length(); i++) {
      JSONObject c = chats().optJSONObject(i);
      if (c.optString("id").equals(root.optString("selected"))) return c;
    }
    return chats().optJSONObject(0);
  }

  JSONObject newChat() {
    JSONObject c =
        Json.obj(
            "id",
            Json.id(),
            "title",
            "新对话",
            "input",
            "",
            "messages",
            new JSONArray(),
            "attachments",
            new JSONArray(),
            "created",
            System.currentTimeMillis());
    chats().put(c);
    Json.put(root, "selected", c.optString("id"));
    return c;
  }

  void save() throws IOException {
    if (corrupt) throw new IOException(loadIssue);
    FileOutputStream out = null;
    try {
      out = file.startWrite();
      out.write(root.toString().getBytes(StandardCharsets.UTF_8));
      file.finishWrite(out);
    } catch (Exception e) {
      if (out != null) file.failWrite(out);
      throw new IOException("保存失败，请检查手机存储空间。", e);
    }
  }

  private SecretKey key() throws Exception {
    KeyStore ks = KeyStore.getInstance("AndroidKeyStore");
    ks.load(null);
    if (ks.containsAlias("potato.credentials"))
      return (SecretKey) ks.getKey("potato.credentials", null);
    KeyGenerator gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
    gen.init(
        new KeyGenParameterSpec.Builder(
                "potato.credentials", KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .build());
    return gen.generateKey();
  }

  void secret(String name, String value) throws Exception {
    var prefs = context.getSharedPreferences("credentials", Context.MODE_PRIVATE);
    if (value.isEmpty()) {
      if (!prefs.edit().remove(name).commit()) throw new IOException("凭据保存失败");
      return;
    }
    Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
    cipher.init(Cipher.ENCRYPT_MODE, key());
    cipher.updateAAD(name.getBytes(StandardCharsets.UTF_8));
    String data =
        Base64.encodeToString(cipher.getIV(), Base64.NO_WRAP)
            + ":"
            + Base64.encodeToString(
                cipher.doFinal(value.getBytes(StandardCharsets.UTF_8)), Base64.NO_WRAP);
    if (!prefs.edit().putString(name, data).commit()) throw new IOException("凭据保存失败");
  }

  String secret(String name) throws Exception {
    String saved =
        context.getSharedPreferences("credentials", Context.MODE_PRIVATE).getString(name, "");
    if (saved.isEmpty()) return "";
    String[] parts = saved.split(":");
    Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
    cipher.init(
        Cipher.DECRYPT_MODE,
        key(),
        new GCMParameterSpec(128, Base64.decode(parts[0], Base64.NO_WRAP)));
    cipher.updateAAD(name.getBytes(StandardCharsets.UTF_8));
    return new String(
        cipher.doFinal(Base64.decode(parts[1], Base64.NO_WRAP)), StandardCharsets.UTF_8);
  }

  String token() throws Exception {
    JSONObject s = settings();
    String endpoint = s.optString("endpoint");
    if (s.optBoolean("cloud")) {
      if (!endpoint.equals(RELAY + "/v1/chat/completions")) throw new IOException("云端凭据不能用于其他服务。");
      return secret("cloud");
    }
    return secret("custom:" + endpoint);
  }
}
