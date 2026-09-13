package top.recodex.potato;

import java.util.*;
import org.json.*;

final class Json {
  static JSONObject obj(Object... pairs) {
    JSONObject o = new JSONObject();
    try {
      for (int i = 0; i < pairs.length; i += 2)
        o.put((String) pairs[i], pairs[i + 1] == null ? JSONObject.NULL : pairs[i + 1]);
    } catch (JSONException e) {
      throw new IllegalArgumentException(e);
    }
    return o;
  }

  static void put(JSONObject o, String key, Object value) {
    try {
      o.put(key, value);
    } catch (JSONException e) {
      throw new IllegalArgumentException(e);
    }
  }

  static JSONArray array(JSONObject o, String key) {
    JSONArray a = o.optJSONArray(key);
    if (a == null) {
      a = new JSONArray();
      put(o, key, a);
    }
    return a;
  }

  static JSONObject object(JSONObject o, String key) {
    JSONObject v = o.optJSONObject(key);
    if (v == null) {
      v = obj();
      put(o, key, v);
    }
    return v;
  }

  static JSONObject copy(JSONObject o) {
    try {
      return new JSONObject(o.toString());
    } catch (JSONException e) {
      throw new IllegalArgumentException(e);
    }
  }

  static JSONArray copy(JSONArray a) {
    try {
      return new JSONArray(a.toString());
    } catch (JSONException e) {
      throw new IllegalArgumentException(e);
    }
  }

  static String id() {
    return UUID.randomUUID().toString();
  }
}
