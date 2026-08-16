import { lazy, Suspense, useEffect, useState, type ReactNode } from "react";
import {
  HashRouter,
  Navigate,
  Route,
  Routes,
  useLocation,
} from "react-router-dom";
import { DesktopHostBridge } from "./components/desktop/DesktopHostBridge";
import { AppShell } from "./components/layout/AppShell";
import { authApi, getAuthToken } from "./lib/api";
import {
  ensureBackendOriginResolver,
  waitForBackendOrigin,
} from "./lib/backendOrigin";
import { notifyDesktopReady } from "./lib/desktop";
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
  useEffect(() => {
    ensureBackendOriginResolver();
    void notifyDesktopReady();
  }, []);
  return (
    <HashRouter>
      <DesktopHostBridge />
      <AuthGate>
        <AppRoutes />
      </AuthGate>
    </HashRouter>
  );
}

/**
 * 设置是覆盖层不是页面:应用内打开时带上 background location,
 * 底下的页面(会话等)保持挂载,模态悬浮其上——对齐 ChatGPT/Codex。
 * 直接深链 /settings(无 background)时退化为整页路由。
 */
function AppRoutes() {
  const location = useLocation();
  const background = (
    location.state as { background?: ReturnType<typeof useLocation> } | null
  )?.background;
  return (
    <>
      <Routes location={background ?? location}>
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
      {background && location.pathname === "/settings" && (
        <Suspense fallback={<SettingsLoading />}>
          <SettingsView />
        </Suspense>
      )}
    </>
  );
}

function PageLoading({
  label,
}: {
  label: "crons.loading" | "skills.loading" | "memory.loading";
}) {
  const { t } = useTranslation();
  return (
    <div className="flex h-full items-center justify-center bg-surface text-sm text-ink-tertiary">
      {t(label)}
    </div>
  );
}

function SettingsLoading() {
  const { t } = useTranslation();
  return (
    <div className="flex h-full items-center justify-center bg-surface text-sm text-ink-tertiary">
      {t("settings.loading")}
    </div>
  );
}

function AuthGate({ children }: { children: ReactNode }) {
  const location = useLocation();
  const [loginRequired, setLoginRequired] = useState(false);

  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        await waitForBackendOrigin();
        if (!active) return;
        const status = await authApi.status();
        if (!active) return;
        setLoginRequired(Boolean(status.enabled && !getAuthToken()));
      } catch {
        if (active) setLoginRequired(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [location.pathname]);

  if (loginRequired && !getAuthToken() && location.pathname !== "/login") {
    return <Navigate to="/login" replace />;
  }
  return children;
}
