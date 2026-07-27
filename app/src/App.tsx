import {
  lazy,
  Suspense,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import {
  HashRouter,
  Navigate,
  Route,
  Routes,
  useLocation,
} from "react-router-dom";
import { AppShell } from "./components/layout/AppShell";
import { authApi, getAuthToken } from "./lib/api";
import { useTranslation } from "./lib/i18n";
import { useThemeInit } from "./lib/theme";
import { ChatView } from "./views/ChatView";
import { LoginView } from "./views/LoginView";

const SettingsView = lazy(() =>
  import("./views/SettingsView").then((module) => ({
    default: module.SettingsView,
  })),
);
const CronsView = lazy(() =>
  import("./views/CronsView").then((module) => ({
    default: module.CronsView,
  })),
);
const InboxView = lazy(() =>
  import("./views/InboxView").then((module) => ({
    default: module.InboxView,
  })),
);
const SkillsView = lazy(() =>
  import("./views/SkillsView").then((module) => ({
    default: module.SkillsView,
  })),
);
const MemoryView = lazy(() =>
  import("./views/MemoryView").then((module) => ({
    default: module.MemoryView,
  })),
);

export function App() {
  useThemeInit();
  return (
    <HashRouter>
      <AuthGate>
        <Routes>
          <Route path="/login" element={<LoginView />} />
          <Route element={<AppShell />}>
            <Route path="/" element={<ChatView />} />
            <Route path="/chat/:chatId" element={<ChatView />} />
            <Route
              path="/settings"
              element={
                <Suspense fallback={<SettingsLoading />}>
                  <SettingsView />
                </Suspense>
              }
            />
            <Route
              path="/crons"
              element={
                <Suspense fallback={<PageLoading label="crons.loading" />}>
                  <CronsView />
                </Suspense>
              }
            />
            <Route
              path="/inbox"
              element={
                <Suspense fallback={<PageLoading label="inbox.loading" />}>
                  <InboxView />
                </Suspense>
              }
            />
            <Route
              path="/skills"
              element={
                <Suspense fallback={<PageLoading label="skills.loading" />}>
                  <SkillsView />
                </Suspense>
              }
            />
            <Route
              path="/memory"
              element={
                <Suspense fallback={<PageLoading label="memory.loading" />}>
                  <MemoryView />
                </Suspense>
              }
            />
          </Route>
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </AuthGate>
    </HashRouter>
  );
}

function PageLoading({
  label,
}: {
  label:
    | "crons.loading"
    | "inbox.loading"
    | "skills.loading"
    | "memory.loading";
}) {
  const { t } = useTranslation();
  return (
    <div className="flex h-full items-center justify-center bg-surface text-sm text-ink-muted">
      {t(label)}
    </div>
  );
}

function SettingsLoading() {
  const { t } = useTranslation();
  return (
    <div className="flex h-full items-center justify-center bg-surface text-sm text-ink-muted">
      {t("settings.loading")}
    </div>
  );
}

function AuthGate({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const location = useLocation();
  const [state, setState] = useState<"checking" | "allowed" | "login">(
    "checking",
  );

  useEffect(() => {
    let active = true;
    void authApi
      .status()
      .then((status) => {
        if (!active) return;
        setState(!status.enabled || getAuthToken() ? "allowed" : "login");
      })
      .catch(() => {
        if (active) setState("allowed");
      });
    return () => {
      active = false;
    };
  }, [location.pathname]);

  if (state === "checking") {
    return (
      <div className="flex h-full items-center justify-center bg-bg text-sm text-ink-muted">
        {t("app.connecting")}
      </div>
    );
  }
  if (
    state === "login" &&
    !getAuthToken() &&
    location.pathname !== "/login"
  ) {
    return <Navigate to="/login" replace />;
  }
  return children;
}
