import { useEffect, useRef, useState, type FormEvent } from "react";
import { ApertureMark } from "../components/ApertureMark";
import { GoogleMark } from "../components/GoogleMark";
import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";
import { authGoogleBegin, authLogin, type AuthError } from "../lib/auth";
import { validateEmail } from "../lib/formValidation";
import { isDesktopShell } from "../lib/ipc";
import { navigateTo } from "../lib/router";

interface SignInPageProps {
  /** Returns to the previous onboarding step. */
  onBack: () => void;
  /** Optional callback upon successful authentication. */
  onSuccess?: () => void;
}

interface FieldErrors {
  email?: string;
  password?: string;
}

interface StatusMessage {
  tone: "info" | "error";
  message: string;
}

const HONEST_SIGNIN_STATUS =
  "Sign-in is not connected yet. Account authentication will be enabled in a later phase.";

const GOOGLE_STATUS =
  "Google sign-in will be connected with the authentication backend in a later phase.";

/** Sign-in only checks presence — it never enforces length on existing passwords. */
function validatePassword(value: string): string | undefined {
  if (value.length === 0) return "Enter your password.";
  return undefined;
}

/**
 * The Phase 21 & Phase 24 sign-in interface.
 *
 * In browser preview mode, it reports honestly that authentication is interface-only.
 * In desktop mode (Tauri), it invokes the secure `auth_login` and `auth_google_begin`
 * commands over Tauri IPC. Raw session tokens are stored in the OS credential manager
 * and are never exposed to frontend state, storage, or error logs.
 */
export function SignInPage({ onBack, onSuccess }: SignInPageProps) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [errors, setErrors] = useState<FieldErrors>({});
  const [submitting, setSubmitting] = useState(false);
  const [status, setStatus] = useState<StatusMessage | null>(null);

  const emailRef = useRef<HTMLInputElement>(null);
  const passwordRef = useRef<HTMLInputElement>(null);
  const submitTimerRef = useRef<number | null>(null);

  // Never leave a scheduled status update behind on unmount.
  useEffect(() => {
    return () => {
      if (submitTimerRef.current !== null) {
        window.clearTimeout(submitTimerRef.current);
      }
    };
  }, []);

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    // Repeat-submission guard while the submission state is active.
    if (submitting) return;

    const nextErrors: FieldErrors = {};
    const emailError = validateEmail(email);
    const passwordError = validatePassword(password);
    if (emailError) nextErrors.email = emailError;
    if (passwordError) nextErrors.password = passwordError;

    setErrors(nextErrors);
    setStatus(null);

    if (nextErrors.email || nextErrors.password) {
      // Keep every entered value — including the password — untouched.
      if (nextErrors.email) {
        emailRef.current?.focus();
      } else {
        passwordRef.current?.focus();
      }
      return;
    }

    setSubmitting(true);

    // When running in desktop Tauri shell, submit via secure IPC command.
    if (isDesktopShell()) {
      try {
        await authLogin({ email, password });
        setPassword("");
        setSubmitting(false);
        if (onSuccess) {
          onSuccess();
        } else {
          navigateTo("home");
        }
      } catch (err: unknown) {
        setSubmitting(false);
        const authErr = err as AuthError;
        if (authErr.code === "invalid_credentials") {
          setStatus({ tone: "error", message: "Invalid email or password." });
        } else if (authErr.code === "email_unverified") {
          setStatus({
            tone: "error",
            message: "Please verify your email address before signing in.",
          });
        } else if (authErr.code === "rate_limited") {
          const mins = Math.ceil((authErr.retryAfterSeconds || 60) / 60);
          setStatus({
            tone: "error",
            message: `Too many attempts. Try again in ${mins} minute${mins === 1 ? "" : "s"}.`,
          });
        } else {
          setStatus({
            tone: "error",
            message:
              authErr.message ||
              "Can't reach the Nexora account service. Check your connection and try again.",
          });
        }
      }
      return;
    }

    // Browser preview mode (non-desktop):
    submitTimerRef.current = window.setTimeout(() => {
      setStatus({ tone: "info", message: HONEST_SIGNIN_STATUS });
      setSubmitting(false);
      submitTimerRef.current = null;
    }, 400);
  };

  const handleGoogleSignIn = async () => {
    if (!isDesktopShell()) {
      setStatus({ tone: "info", message: GOOGLE_STATUS });
      return;
    }

    setSubmitting(true);
    setStatus({ tone: "info", message: "Opening browser for Google sign-in..." });
    try {
      await authGoogleBegin();
      setSubmitting(false);
      if (onSuccess) {
        onSuccess();
      } else {
        navigateTo("home");
      }
    } catch (err: unknown) {
      setSubmitting(false);
      const authErr = err as AuthError;
      if (authErr.code === "email_conflict") {
        setStatus({
          tone: "error",
          message:
            "An account with this email already exists. Sign in with your password to link Google.",
        });
      } else if (authErr.code === "cancelled") {
        setStatus({ tone: "info", message: "Google sign-in was cancelled." });
      } else {
        setStatus({
          tone: "error",
          message: authErr.message || "Google sign-in could not be completed.",
        });
      }
    }
  };

  return (
    <div className="onboarding">
      <div className="onboarding__card signin__card">
        <div className="welcome__brand">
          <span className="welcome__mark" aria-hidden="true">
            <ApertureMark size={22} />
          </span>
          <span className="welcome__wordmark">nexora</span>
        </div>

        <ol className="onboarding__steps" aria-label="Onboarding progress">
          <li className="onboarding__step onboarding__step--done">Welcome</li>
          <li className="onboarding__step onboarding__step--current" aria-current="step">
            Sign in
          </li>
        </ol>

        <header className="signin__head">
          <h1 className="onboarding__title">Welcome back</h1>
          <p className="onboarding__lead">
            Sign in to continue into Nexora. Account authentication is being
            connected in a later phase — this is the interface for it, and the
            local workspace keeps working without an account.
          </p>
        </header>

        <form className="signin__form" aria-label="Sign in form" onSubmit={handleSubmit} noValidate>
          <div className="field">
            <label className="field__label" htmlFor="signin-email">
              Email address
            </label>
            <input
              ref={emailRef}
              id="signin-email"
              className="field__input field__input--ui"
              type="email"
              autoComplete="email"
              inputMode="email"
              spellCheck={false}
              value={email}
              disabled={submitting}
              aria-invalid={errors.email ? true : undefined}
              aria-describedby={errors.email ? "signin-email-error" : undefined}
              onChange={(event) => {
                setEmail(event.target.value);
                if (errors.email) {
                  setErrors((current) => ({ ...current, email: undefined }));
                }
              }}
            />
            {errors.email ? (
              <p id="signin-email-error" className="field__error" role="alert">
                {errors.email}
              </p>
            ) : null}
          </div>

          <div className="field">
            <label className="field__label" htmlFor="signin-password">
              Password
            </label>
            <div className="field__row">
              <input
                ref={passwordRef}
                id="signin-password"
                className="field__input field__input--ui"
                type={showPassword ? "text" : "password"}
                autoComplete="current-password"
                value={password}
                disabled={submitting}
                aria-invalid={errors.password ? true : undefined}
                aria-describedby={errors.password ? "signin-password-error" : undefined}
                onChange={(event) => {
                  setPassword(event.target.value);
                  if (errors.password) {
                    setErrors((current) => ({ ...current, password: undefined }));
                  }
                }}
              />
              <button
                type="button"
                className="signin__visibility"
                aria-label={showPassword ? "Hide password" : "Show password"}
                aria-pressed={showPassword}
                aria-controls="signin-password"
                title={showPassword ? "Hide password" : "Show password"}
                onClick={() => setShowPassword((current) => !current)}
              >
                <Icon name={showPassword ? "eye-off" : "eye"} size={14} />
              </button>
            </div>
            {errors.password ? (
              <p id="signin-password-error" className="field__error" role="alert">
                {errors.password}
              </p>
            ) : null}
          </div>

          <button type="submit" className="button button--primary signin__submit" disabled={submitting} aria-busy={submitting}>
            {submitting ? "Signing in…" : "Sign in"}
          </button>
        </form>

        <div className="signin__status" role="status" aria-live="polite">
          {status ? <p className="signin__status-text">{status.message}</p> : null}
        </div>

        <div className="signin__divider" aria-hidden="true">
          <span>or</span>
        </div>

        <button
          type="button"
          className="google-btn"
          disabled={submitting}
          onClick={handleGoogleSignIn}
        >
          <GoogleMark size={18} />
          <span>Sign in with Google</span>
        </button>

        <footer className="signin__footer">
          <div className="signin__links">
            <RouterLink to="recover" className="signin__link">
              Forgot password?
            </RouterLink>
            <RouterLink to="signup" className="signin__link">
              Create account
            </RouterLink>
          </div>
          <button type="button" className="button button--ghost button--sm" onClick={onBack}>
            Back to onboarding
          </button>
        </footer>
      </div>
    </div>
  );
}
