import * as Dialog from "@radix-ui/react-dialog";
import { useTranslation } from "../../lib/i18n";
import { Button } from "./Button";

/**
 * 确认对话框，取代 window.confirm（6 处）。受控 open + 回调式。
 * 动效走 global.css 的 qp-overlay / qp-pop。
 */
export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  description?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  tone?: "default" | "danger";
  busy?: boolean;
  onConfirm: () => void;
  onOpenChange: (open: boolean) => void;
}

export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel,
  cancelLabel,
  tone = "default",
  busy,
  onConfirm,
  onOpenChange,
}: ConfirmDialogProps) {
  const { t } = useTranslation();
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="qp-overlay fixed inset-0 z-40 bg-ink/25 backdrop-blur-[1px]" />
        <Dialog.Content className="qp-pop fixed left-1/2 top-1/2 z-50 w-[calc(100%-2rem)] max-w-sm -translate-x-1/2 -translate-y-1/2 rounded-[var(--radius-lg)] border border-line bg-raised p-5 shadow-[var(--shadow-lg)] outline-none">
          <Dialog.Title className="text-sm font-semibold text-ink">
            {title}
          </Dialog.Title>
          {description && (
            <Dialog.Description className="mt-1.5 text-sm leading-6 text-ink-secondary">
              {description}
            </Dialog.Description>
          )}
          <div className="mt-5 flex justify-end gap-2">
            <Button
              variant="ghost"
              size="sm"
              disabled={busy}
              onClick={() => onOpenChange(false)}
            >
              {cancelLabel ?? t("common.cancel")}
            </Button>
            <Button
              variant={tone === "danger" ? "danger" : "primary"}
              size="sm"
              disabled={busy}
              onClick={onConfirm}
              className={
                tone === "danger"
                  ? "border border-danger/30 bg-danger-soft"
                  : undefined
              }
            >
              {confirmLabel ?? t("common.confirm")}
            </Button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
