import { type ReactNode } from "react";
import BackendLoadingPage from "./BackendLoadingPage";
import useBackendReadyPolling from "./useBackendReadyPolling";

interface Props {
  children: ReactNode;
}

export default function BackendReadyGate({ children }: Props) {
  const {
    shouldGate,
    status,
    errorMessage,
    retry,
  } = useBackendReadyPolling();

  // Browser mode, or Tauri after it has navigated to the backend-hosted console.
  if (!shouldGate) {
    return <>{children}</>;
  }

  // The shell reveals this WebView as soon as the bootstrap page has painted,
  // so the normal startup path must show a real splash — rendering null here
  // would put a blank window on screen for the whole backend cold start.
  if (status === "checking" || status === "ready") {
    return <BackendLoadingPage state="loading" />;
  }

  return <BackendLoadingPage errorMessage={errorMessage} onRetry={retry} />;
}
