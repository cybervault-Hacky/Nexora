import { useEffect, useId, useRef, useState, type FormEvent } from "react";
import { ApertureMark } from "./ApertureMark";
import { GoogleMark } from "./GoogleMark";
import { Icon } from "./Icon";
import {
  getDeveloperDiagnostics,
  type AuthError,
  type AuthStatusResponse,
  type AuthUser,
  type LoginPayload,
  type RegisterPayload,
} from "../lib/auth";
import { isDesktopShell } from "../lib/ipc";
import {
  PASSWORD_MIN_LENGTH,
  validateEmail,
  validateFullName,
  validateNewPassword,
  validatePasswordConfirmation,
} from "../lib/formValidation";

function validateToken(value: string, label: string): string | undefined {
  if (value.trim().length === 0) return `Enter your ${label}.`;
  return undefined;
}

export type AuthModalMode = "signin" | "signup" | "recover" | "reset" | "verify";

interface AuthModalProps {
  open: boolean;
  initialMode?: AuthModalMode;
  reason?: string | null;
  preservedPrompt?: string;
  onClose: () => void;
  onLogin: (payload: LoginPayload) => Promise<AuthUser>;
  onRegister: (payload: RegisterPayload) => Promise<AuthStatusResponse>;
  onVerifyEmail: (token: string) => Promise<AuthStatusResponse>;
  onResendVerification: (email: string) => Promise<AuthStatusResponse>;
  onRecoverPassword: (email: string) => Promise<AuthStatusResponse>;
  onResetPassword: (token: string, password: string) => Promise<AuthStatusResponse>;
  onGoogleSignIn: () => Promise<AuthUser>;
  onCancelGoogle: () => Promise<void>;
}

export function AuthModal({
  open,
  initialMode = "signin",
  reason = null,
  preservedPrompt,
  onClose,
  onLogin,
  onRegister,
  onVerifyEmail,
  onResendVerification,
  onRecoverPassword,
  onResetPassword,
  onGoogleSignIn,
  onCancelGoogle,
}: AuthModalProps) {
  const titleId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);

  const [mode, setMode] = useState<AuthModalMode>(initialMode);
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [googleBusy, setGoogleBusy] = useState(false);
  const [authServerConfigured, setAuthServerConfigured] = useState<boolean | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setMode(initialMode);
      setError(null);
      setNotice(null);
      setPassword("");
      setConfirmPassword("");
      setGoogleBusy(false);

      if (!isDesktopShell()) {
        setAuthServerConfigured(false);
      } else {
        void getDeveloperDiagnostics()
          .then((diag) => {
            if (diag && typeof diag === "object" && "authServiceUrl" in diag) {
              setAuthServerConfigured(Boolean(diag.authServiceUrl && diag.authServiceUrl.trim()));
            } else {
              // Mocked environments without diagnostics let real IPC handlers decide.
              setAuthServerConfigured(null);
            }
          })
          .catch(() => {
            setAuthServerConfigured(null);
          });
      }
    }
  }, [open, initialMode]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        if (googleBusy) {
          void onCancelGoogle();
          setGoogleBusy(false);
        } else {
          onClose();
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, googleBusy, onCancelGoogle, onClose]);

  if (!open) return null;

  const switchMode = (next: AuthModalMode) => {
    setMode(next);
    setError(null);
    setNotice(null);
  };

  async function handleSignIn(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setNotice(null);
    const emailErr = validateEmail(email);
    if (emailErr) {
      setError(emailErr);
      return;
    }
    if (!password) {
      setError("Enter your password.");
      return;
    }
    setBusy(true);
    try {
      await onLogin({ email: email.trim(), password });
      setPassword("");
      onClose();
    } catch (err) {
      const authErr = err as AuthError;
      if (authErr.code === "email_not_verified") {
        setNotice("Verify your email address before signing in.");
        setMode("verify");
      } else {
        setError(authErr.message || "Invalid email or password.");
      }
    } finally {
      setBusy(false);
    }
  }

  async function handleSignUp(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setNotice(null);
    const nameErr = validateFullName(name);
    if (nameErr) {
      setError(nameErr);
      return;
    }
    const emailErr = validateEmail(email);
    if (emailErr) {
      setError(emailErr);
      return;
    }
    const passErr = validateNewPassword(password);
    if (passErr) {
      setError(passErr);
      return;
    }
    const confirmErr = validatePasswordConfirmation(password, confirmPassword);
    if (confirmErr) {
      setError(confirmErr);
      return;
    }
    setBusy(true);
    try {
      const res = await onRegister({
        name: name.trim(),
        email: email.trim(),
        password,
      });
      setPassword("");
      setConfirmPassword("");
      setNotice(res.message || "Account created. Check your email to verify your address.");
      setMode("verify");
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Could not create account.");
    } finally {
      setBusy(false);
    }
  }

  async function handleVerify(e: FormEvent) {
    e.preventDefault();
    setError(null);
    const tokErr = validateToken(token, "verification token");
    if (tokErr) {
      setError(tokErr);
      return;
    }
    setBusy(true);
    try {
      const res = await onVerifyEmail(token.trim());
      setToken("");
      setNotice(res.message || "Email verified. Sign in with your email and password.");
      setMode("signin");
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Verification token is invalid or expired.");
    } finally {
      setBusy(false);
    }
  }

  async function handleResend() {
    setError(null);
    const emailErr = validateEmail(email);
    if (emailErr) {
      setError("Enter your email address to resend verification.");
      return;
    }
    setBusy(true);
    try {
      const res = await onResendVerification(email.trim());
      setNotice(res.message || "Verification message requested.");
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Could not resend verification email.");
    } finally {
      setBusy(false);
    }
  }

  async function handleRecover(e: FormEvent) {
    e.preventDefault();
    setError(null);
    const emailErr = validateEmail(email);
    if (emailErr) {
      setError(emailErr);
      return;
    }
    setBusy(true);
    try {
      const res = await onRecoverPassword(email.trim());
      setNotice(res.message || "If an account exists for that email, a reset token has been sent.");
      setMode("reset");
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Could not request password reset.");
    } finally {
      setBusy(false);
    }
  }

  async function handleReset(e: FormEvent) {
    e.preventDefault();
    setError(null);
    const tokErr = validateToken(token, "reset token");
    if (tokErr) {
      setError(tokErr);
      return;
    }
    const passErr = validateNewPassword(password);
    if (passErr) {
      setError(passErr);
      return;
    }
    setBusy(true);
    try {
      const res = await onResetPassword(token.trim(), password);
      setToken("");
      setPassword("");
      setNotice(res.message || "Password updated. Sign in with your new password.");
      setMode("signin");
    } catch (err) {
      const authErr = err as AuthError;
      setError(authErr.message || "Reset token is invalid or expired.");
    } finally {
      setBusy(false);
    }
  }

  async function handleGoogle() {
    setError(null);
    setNotice(null);
    setGoogleBusy(true);
    try {
      await onGoogleSignIn();
      onClose();
    } catch (err) {
      const authErr = err as AuthError;
      if (authErr.code !== "cancelled") {
        setError(authErr.message || "Google sign-in could not be completed.");
      }
    } finally {
      setGoogleBusy(false);
    }
  }

  return (
    <div
      className="auth-modal-backdrop"
      role="presentation"
      data-testid="auth-modal-backdrop"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && !googleBusy) {
          onClose();
        }
      }}
    >
      <div
        ref={dialogRef}
        className="auth-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        data-testid="auth-modal"
      >
        <div className="auth-modal__header">
          <div className="auth-modal__brand">
            <ApertureMark size={18} />
            <span>Nexora Account</span>
          </div>
          <button
            type="button"
            className="auth-modal__close"
            aria-label="Close sign-in dialog"
            onClick={() => {
              if (googleBusy) void onCancelGoogle();
              onClose();
            }}
          >
            <Icon name="x" size={16} />
          </button>
        </div>

        <h2 id={titleId} className="auth-modal__title">
          {mode === "signin" && "Sign in to Nexora"}
          {mode === "signup" && "Create your Nexora account"}
          {mode === "verify" && "Verify your email address"}
          {mode === "recover" && "Reset your password"}
          {mode === "reset" && "Set a new password"}
        </h2>

        <p className="auth-modal__subtitle">
          Local AI inference and conversation history stay on your machine. Sign in is optional for local chat.
        </p>

        {reason ? (
          <div className="auth-modal__reason" role="note" data-testid="auth-modal-reason">
            <Icon name="shield" size={14} />
            <span>{reason}</span>
          </div>
        ) : null}

        {preservedPrompt && preservedPrompt.trim().length > 0 && (
          <div className="auth-modal__preserved" data-testid="auth-preserved-prompt">
            <Icon name="message" size={14} />
            <span>Your current draft will be preserved when you close or complete sign-in.</span>
          </div>
        )}

        {authServerConfigured === false && (
          <div className="auth-modal__unconfigured" role="note" data-testid="auth-backend-note">
            <span>
              Account authentication is not configured in this local build. Close this dialog anytime to continue chatting locally.
            </span>
          </div>
        )}

        {(mode === "signin" || mode === "signup") && (
          <div className="auth-modal__tabs" role="tablist" aria-label="Authentication mode">
            <button
              type="button"
              role="tab"
              aria-selected={mode === "signin"}
              className={`auth-modal__tab${mode === "signin" ? " auth-modal__tab--active" : ""}`}
              onClick={() => switchMode("signin")}
            >
              Sign in
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={mode === "signup"}
              className={`auth-modal__tab${mode === "signup" ? " auth-modal__tab--active" : ""}`}
              onClick={() => switchMode("signup")}
            >
              Create account
            </button>
          </div>
        )}

        {error && (
          <div className="auth-modal__alert auth-modal__alert--error" role="alert">
            <Icon name="alert" size={15} />
            <span>{error}</span>
          </div>
        )}

        {notice && (
          <div className="auth-modal__alert auth-modal__alert--info" role="status">
            <Icon name="check" size={15} />
            <span>{notice}</span>
          </div>
        )}

        {(mode === "signin" || mode === "signup") && (
          <>
            {googleBusy ? (
              <div className="auth-modal__google-wait">
                <span>Complete Google sign-in in your browser…</span>
                <button
                  type="button"
                  className="button button--ghost button--sm"
                  onClick={() => {
                    void onCancelGoogle();
                    setGoogleBusy(false);
                  }}
                >
                  Cancel
                </button>
              </div>
            ) : (
              <button
                type="button"
                className="auth-modal__google-btn"
                onClick={handleGoogle}
                disabled={busy}
              >
                <GoogleMark size={16} />
                <span>Continue with Google</span>
              </button>
            )}

            <div className="auth-modal__divider">
              <span>or continue with email</span>
            </div>
          </>
        )}

        {mode === "signin" && (
          <form className="auth-modal__form" onSubmit={handleSignIn} noValidate>
            <label className="auth-modal__field">
              <span>Email address</span>
              <input
                className="field__input"
                type="email"
                autoComplete="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                disabled={busy || googleBusy}
              />
            </label>

            <label className="auth-modal__field">
              <span>Password</span>
              <input
                className="field__input"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="••••••••"
                disabled={busy || googleBusy}
              />
            </label>

            <button
              type="submit"
              className="button button--primary auth-modal__submit"
              disabled={busy || googleBusy}
            >
              {busy ? "Signing in…" : "Sign in"}
            </button>

            <div className="auth-modal__footer-links">
              <button
                type="button"
                className="auth-modal__link"
                onClick={() => switchMode("recover")}
              >
                Forgot password?
              </button>
              <button
                type="button"
                className="auth-modal__link"
                onClick={() => switchMode("verify")}
              >
                Have a verification code?
              </button>
            </div>
          </form>
        )}

        {mode === "signup" && (
          <form className="auth-modal__form" onSubmit={handleSignUp} noValidate>
            <label className="auth-modal__field">
              <span>Name</span>
              <input
                className="field__input"
                type="text"
                autoComplete="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Ada Lovelace"
                disabled={busy || googleBusy}
              />
            </label>

            <label className="auth-modal__field">
              <span>Email address</span>
              <input
                className="field__input"
                type="email"
                autoComplete="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                disabled={busy || googleBusy}
              />
            </label>

            <label className="auth-modal__field">
              <span>Password</span>
              <input
                className="field__input"
                type="password"
                autoComplete="new-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={`At least ${PASSWORD_MIN_LENGTH} characters`}
                disabled={busy || googleBusy}
              />
            </label>

            <label className="auth-modal__field">
              <span>Confirm password</span>
              <input
                className="field__input"
                type="password"
                autoComplete="new-password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                placeholder="Re-enter password"
                disabled={busy || googleBusy}
              />
            </label>

            <button
              type="submit"
              className="button button--primary auth-modal__submit"
              disabled={busy || googleBusy}
            >
              {busy ? "Creating account…" : "Create account"}
            </button>
          </form>
        )}

        {mode === "verify" && (
          <form className="auth-modal__form" onSubmit={handleVerify} noValidate>
            <label className="auth-modal__field">
              <span>Email address</span>
              <input
                className="field__input"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                disabled={busy}
              />
            </label>

            <label className="auth-modal__field">
              <span>Verification token</span>
              <input
                className="field__input"
                type="text"
                value={token}
                onChange={(e) => setToken(e.target.value)}
                placeholder="Paste verification token from your email"
                disabled={busy}
              />
            </label>

            <button
              type="submit"
              className="button button--primary auth-modal__submit"
              disabled={busy}
            >
              {busy ? "Verifying…" : "Verify email"}
            </button>

            <div className="auth-modal__footer-links">
              <button
                type="button"
                className="auth-modal__link"
                onClick={handleResend}
                disabled={busy}
              >
                Resend verification email
              </button>
              <button
                type="button"
                className="auth-modal__link"
                onClick={() => switchMode("signin")}
              >
                Back to Sign in
              </button>
            </div>
          </form>
        )}

        {mode === "recover" && (
          <form className="auth-modal__form" onSubmit={handleRecover} noValidate>
            <label className="auth-modal__field">
              <span>Email address</span>
              <input
                className="field__input"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                disabled={busy}
              />
            </label>

            <button
              type="submit"
              className="button button--primary auth-modal__submit"
              disabled={busy}
            >
              {busy ? "Requesting…" : "Send reset token"}
            </button>

            <div className="auth-modal__footer-links">
              <button
                type="button"
                className="auth-modal__link"
                onClick={() => switchMode("reset")}
              >
                Already have a reset token?
              </button>
              <button
                type="button"
                className="auth-modal__link"
                onClick={() => switchMode("signin")}
              >
                Back to Sign in
              </button>
            </div>
          </form>
        )}

        {mode === "reset" && (
          <form className="auth-modal__form" onSubmit={handleReset} noValidate>
            <label className="auth-modal__field">
              <span>Reset token</span>
              <input
                className="field__input"
                type="text"
                value={token}
                onChange={(e) => setToken(e.target.value)}
                placeholder="Paste reset token"
                disabled={busy}
              />
            </label>

            <label className="auth-modal__field">
              <span>New password</span>
              <input
                className="field__input"
                type="password"
                autoComplete="new-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={`At least ${PASSWORD_MIN_LENGTH} characters`}
                disabled={busy}
              />
            </label>

            <button
              type="submit"
              className="button button--primary auth-modal__submit"
              disabled={busy}
            >
              {busy ? "Updating…" : "Update password"}
            </button>

            <div className="auth-modal__footer-links">
              <button
                type="button"
                className="auth-modal__link"
                onClick={() => switchMode("signin")}
              >
                Back to Sign in
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
