/** One-time browser storage migration from the QwenPaw brand namespace. */

const STORAGE_KEYS: ReadonlyArray<readonly [string, string]> = [
  ["qwenpaw_theme", "potato_theme"],
  ["qwenpaw_custom_themes", "potato_custom_themes"],
  ["qwenpaw_language", "potato_language"],
  ["qwenpaw_auth_token", "potato_auth_token"],
  ["qwenpaw_desktop_close_action", "potato_desktop_close_action"],
  ["qwenpaw_project_last", "potato_project_last"],
  ["qwenpaw_project_recent", "potato_project_recent"],
  ["qwenpaw_sidebar_collapsed", "potato_sidebar_collapsed"],
  ["qwenpaw_pending_chat_session", "potato_pending_chat_session"],
  ["qwenpaw.toolDetail", "potato.toolDetail"],
  ["qwenpaw.contextUsage", "potato.contextUsage"],
  ["qwenpaw.toolDebug", "potato.toolDebug"],
];

function migrateStorage(storage: Storage | undefined): void {
  if (!storage) return;
  for (const [legacyKey, potatoKey] of STORAGE_KEYS) {
    if (storage.getItem(potatoKey) !== null) continue;
    const value = storage.getItem(legacyKey);
    if (value !== null) storage.setItem(potatoKey, value);
  }

  const legacyProjectPrefix = "qwenpaw_project_session:";
  const keys = Array.from({ length: storage.length }, (_, index) =>
    storage.key(index),
  ).filter((key): key is string => key !== null);
  for (const key of keys) {
    if (!key.startsWith(legacyProjectPrefix)) continue;
    const potatoKey = `potato_project_session:${key.slice(legacyProjectPrefix.length)}`;
    if (storage.getItem(potatoKey) === null) {
      const value = storage.getItem(key);
      if (value !== null) storage.setItem(potatoKey, value);
    }
  }
}

try {
  migrateStorage(window.localStorage);
  migrateStorage(window.sessionStorage);
} catch {
  // Storage can be unavailable in hardened browser/webview configurations.
}
