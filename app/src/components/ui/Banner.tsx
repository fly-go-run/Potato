import { AlertCircle, TriangleAlert, X } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "../../lib/i18n";

export function Banner({
  tone,
  children,
  actions,
  onDismiss,
}: {
  tone: "danger" | "warn";
  children: ReactNode;
  actions?: ReactNode;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();
  const Icon = tone === "danger" ? AlertCircle : TriangleAlert;

  return (
    <section
      role={tone === "danger" ? "alert" : "status"}
      className={`mx-auto mt-3 flex w-[calc(100%-3rem)] max-w-3xl items-start gap-2 rounded-md px-3 py-2 text-xs ${
        tone === "danger"
          ? "bg-danger-soft text-danger"
          : "bg-warn/10 text-warn"
      }`}
    >
      <Icon size={15} className="mt-0.5 shrink-0" />
      <div className="min-w-0 flex-1">
        <div className="break-words">{children}</div>
        {actions && <div className="mt-2 flex flex-wrap gap-2">{actions}</div>}
      </div>
      <button
        type="button"
        title={t("chat.closeNotice")}
        onClick={onDismiss}
        className="shrink-0 rounded-sm p-0.5 transition-colors hover:bg-surface/50"
      >
        <X size={14} />
      </button>
    </section>
  );
}
