import { useEffect, useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { authApi, setAuthToken, type AuthStatus } from "../lib/api";
import { useTranslation } from "../lib/i18n";

export function LoginView() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void authApi
      .status()
      .then((value) => {
        setStatus(value);
        if (!value.enabled) navigate("/", { replace: true });
      })
      .catch((reason: unknown) =>
        setError(reason instanceof Error ? reason.message : String(reason)),
      );
  }, [navigate]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!status || !username.trim() || !password) return;
    setSubmitting(true);
    setError(null);
    try {
      const result = await authApi.authenticate(
        status.has_users ? "login" : "register",
        username.trim(),
        password,
      );
      setAuthToken(result.token);
      navigate("/", { replace: true });
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="flex h-full items-center justify-center bg-bg px-6">
      <form
        onSubmit={(event) => void submit(event)}
        className="w-full max-w-80 rounded-lg border border-line bg-surface p-6 shadow-sm"
      >
        <h1 className="text-center text-lg font-medium text-ink">QwenPaw</h1>
        <p className="mt-1 text-center text-sm text-ink-muted">
          {status?.has_users === false
            ? t("login.createLocalAccount")
            : t("login.signInWorkspace")}
        </p>

        <label className="mt-6 block text-xs text-ink-secondary">
          {t("login.username")}
          <input
            value={username}
            onChange={(event) => setUsername(event.target.value)}
            autoComplete="username"
            autoFocus
            className="mt-1.5 block w-full rounded-md border border-line bg-surface px-3 py-2 text-sm text-ink outline-none focus:border-line-strong"
          />
        </label>
        <label className="mt-4 block text-xs text-ink-secondary">
          {t("login.password")}
          <input
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            autoComplete={
              status?.has_users === false ? "new-password" : "current-password"
            }
            className="mt-1.5 block w-full rounded-md border border-line bg-surface px-3 py-2 text-sm text-ink outline-none focus:border-line-strong"
          />
        </label>

        {error && (
          <div className="mt-4 rounded-md bg-danger-soft px-3 py-2 text-xs text-danger">
            {error}
          </div>
        )}

        <button
          type="submit"
          disabled={
            !status || submitting || !username.trim() || password.length === 0
          }
          className="mt-5 w-full rounded-md bg-accent px-3 py-2 text-sm font-medium text-surface transition-colors hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-40"
        >
          {submitting
            ? t("login.processing")
            : status?.has_users === false
              ? t("login.createAccount")
              : t("login.signIn")}
        </button>
      </form>
    </div>
  );
}
