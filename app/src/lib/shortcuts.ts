export function isMacPlatform(platform?: string): boolean {
  const value =
    platform ??
    (typeof navigator === "undefined" ? "" : navigator.platform);
  return /mac|iphone|ipad|ipod/i.test(value);
}

export function shortcutModifier(platform?: string): "⌘" | "Ctrl" {
  return isMacPlatform(platform) ? "⌘" : "Ctrl";
}

export function shortcutLabel(key: string, platform?: string): string {
  const modifier = shortcutModifier(platform);
  return modifier === "⌘" ? `${modifier}${key}` : `${modifier}+${key}`;
}

export function isPrimaryShortcut(
  event: Pick<KeyboardEvent, "ctrlKey" | "metaKey">,
  platform?: string,
): boolean {
  return isMacPlatform(platform) ? event.metaKey : event.ctrlKey;
}
