import { useTranslation } from "react-i18next";
import styles from "./BackendLoadingPage.module.less";

interface BackendLoadingPageProps {
  errorMessage?: string;
  onRetry?: () => void;
}

export default function BackendLoadingPage({
  errorMessage,
  onRetry,
}: BackendLoadingPageProps) {
  const { t } = useTranslation();

  return (
    <div className={styles.page}>
      <div className={styles.card}>
        <img src="/qwenpaw.png" alt="Potato" className={styles.logo} />
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
