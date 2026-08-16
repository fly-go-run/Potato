import { useTranslation } from "react-i18next";
import styles from "./BackendLoadingPage.module.less";

interface BackendLoadingPageProps {
  state?: "loading" | "error";
  errorMessage?: string;
  onRetry?: () => void;
}

export default function BackendLoadingPage({
  state = "error",
  errorMessage,
  onRetry,
}: BackendLoadingPageProps) {
  const { t } = useTranslation();

  if (state === "loading") {
    return (
      <div className={styles.page}>
        <div className={styles.card}>
          <img src="/potato.png" alt="" className={styles.logo} />
          <p className={styles.brand}>Potato</p>
          <div className={styles.loadingRow}>
            <span className={styles.spinner} aria-hidden="true" />
            <span className={styles.loadingText}>
              {t("startup.starting", "Starting backend...")}
            </span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className={styles.page}>
      <div className={styles.card}>
        <img src="/potato.png" alt="" className={styles.logo} />
        <p className={styles.brand}>Potato</p>
        <h1 className={styles.title}>
          {t("startup.error", "Backend failed to start.")}
        </h1>
        <p className={styles.hint}>
          {t(
            "startup.errorHint",
            "The backend process could not be launched. Check application logs for details.",
          )}
        </p>
        {errorMessage && (
          <details className={styles.details}>
            <summary className={styles.summary}>
              {t("startup.errorDetails", "Show error details")}
            </summary>
            <pre className={styles.errorDetails}>{errorMessage}</pre>
          </details>
        )}
        <button className={styles.retryButton} onClick={onRetry} type="button">
          {t("startup.retry", "Retry")}
        </button>
      </div>
    </div>
  );
}
