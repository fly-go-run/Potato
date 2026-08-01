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

  // Rust owns the healthy startup path and keeps this WebView hidden until the
  // real app is loaded. The bootstrap renders nothing during normal startup;
  // it only exists as a recoverable diagnostic surface when startup fails.
  if (status === "checking" || status === "ready") return null;

  return <BackendLoadingPage errorMessage={errorMessage} onRetry={retry} />;
}
