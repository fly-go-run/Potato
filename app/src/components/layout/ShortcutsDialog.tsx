import * as Dialog from "@radix-ui/react-dialog";
import { useTranslation, type TranslationKey } from "../../lib/i18n";
import { shortcutLabel } from "../../lib/shortcuts";

interface ShortcutRow {
  label: TranslationKey;
  keys: string;
}

interface ShortcutGroup {
  title: TranslationKey;
  shortcuts: ShortcutRow[];
}

/** 单一数据源:⌘/ 弹层和设置「快捷键」分区共用。 */
export function shortcutGroups(): ShortcutGroup[] {
  return [
    {
      title: "shortcuts.group.navigation",
      shortcuts: [
        { label: "shortcuts.newChat", keys: shortcutLabel("N") },
        { label: "shortcuts.commandPalette", keys: shortcutLabel("K") },
        { label: "shortcuts.toggleSidebar", keys: shortcutLabel("B") },
        { label: "shortcuts.openSettings", keys: shortcutLabel(",") },
        { label: "shortcuts.showShortcuts", keys: shortcutLabel("/") },
      ],
    },
    {
      title: "shortcuts.group.chat",
      shortcuts: [
        { label: "shortcuts.send", keys: "Enter" },
        { label: "shortcuts.newLine", keys: "Shift+Enter" },
        { label: "shortcuts.closeOverlay", keys: "Esc" },
      ],
    },
  ];
}

/** 快捷键分组列表(纯展示),供弹层与设置分区嵌用。 */
export function ShortcutList() {
  const { t } = useTranslation();
  return (
    <div className="space-y-4">
      {shortcutGroups().map((group) => (
        <section key={group.title}>
          <h2 className="mb-1 px-1 text-[11px] text-ink-tertiary">
            {t(group.title)}
          </h2>
          <div className="divide-y divide-line">
            {group.shortcuts.map((shortcut) => (
              <div
                key={shortcut.label}
                className="flex items-center justify-between gap-4 px-1 py-2"
              >
                <span className="text-sm text-ink-secondary">
                  {t(shortcut.label)}
                </span>
                <kbd className="shrink-0 rounded border border-line bg-bubble-tool px-1.5 py-0.5 text-[11px] text-ink-secondary">
                  {shortcut.keys}
                </kbd>
              </div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

export function ShortcutsDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-overlay" />
        <Dialog.Content className="qp-pop fixed left-1/2 top-1/2 z-50 w-[calc(100%-2rem)] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-[var(--radius-lg)] border border-line bg-raised p-5 shadow-[var(--shadow-lg)] outline-none">
          <Dialog.Title className="text-sm font-semibold text-ink">
            {t("shortcuts.title")}
          </Dialog.Title>
          <Dialog.Description className="sr-only">
            {t("shortcuts.description")}
          </Dialog.Description>
          <div className="mt-4">
            <ShortcutList />
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
