import { useEffect, useRef, useState, type FormEvent } from "react";
import { ApertureMark } from "../components/ApertureMark";
import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";
import { authRegister, type AuthError } from "../lib/auth";
import {
  PASSWORD_MIN_LENGTH,
  validateEmail,
  validateFullName,
  validateNewPassword,
  validatePasswordConfirmation,
} from "../lib/formValidation";
import { isDesktopShell } from "../lib/ipc";
import { navigateTo } from "../lib/router";

interface CreateAccountPageProps {
  /** Returns to the sign-in page. */
  onBack: () => void;
}

interface FieldErrors {
  name?: string;
  email?: string;
  password?: string;
  confirm?: string;
}

const HONEST_SIGNUP_STATUS =
  "Account creation is not connected yet. Your details have not been submitted.";

/**
 * The Phase 22 create-account interface.
 *
 * Frontend behaviour only: validation and submission run locally. No account
 * is created, no email is sent, and entered details are never logged, stored,
 * or transmitted. After a completed submission the password fields are cleared
 * (validation failures never clear what you typed).
 */
export function CreateAccountPage({ onBack }: CreateAccountPageProps) {
  const [fullName, setFullName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [errors, setErrors] = useState<FieldErrors>({});
  const [submitting, setSubmitting] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const nameRef = useRef<HTMLInputElement>(null);
  const emailRef = useRef<HTMLInputElement>(null);
  const passwordRef = useRef<HTMLInputElement>(null);
  const confirmRef = useRef<HTMLInputElement>(null);
  const submitTimerRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (submitTimerRef.current !== null) {
        window.clearTimeout(submitTimerRef.current);
      }
    };
  }, []);

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    // Repeat-submission guard while the local submission state is active.
    if (submitting) return;

    const nextErrors: FieldErrors = {};
    const nameError = validateFullName(fullName);
    const emailError = validateEmail(email);
    const passwordError = validateNewPassword(password);
    const confirmError = validatePasswordConfirmation(password, confirm);
    if (nameError) nextErrors.name = nameError;
    if (emailError) nextErrors.email = emailError;
    if (passwordError) nextErrors.password = passwordError;
    if (confirmError) nextErrors.confirm = confirmError;

    setErrors(nextErrors);
    setStatus(null);

    if (nextErrors.name || nextErrors.email || nextErrors.password || nextErrors.confirm) {
      // Keep every entered value — including passwords — untouched.
      if (nextErrors.name) nameRef.current?.focus();
      else if (nextErrors.email) emailRef.current?.focus();
      else if (nextErrors.password) passwordRef.current?.focus();
      else confirmRef.current?.focus();
      return;
    }

    setSubmitting(true);

    if (isDesktopShell()) {
      try {
        await authRegister({ name: fullName, email, password });
        setPassword("");
        setConfirm("");
        setShowPassword(false);
        setShowConfirm(false);
        setSubmitting(false);
        navigateTo("verify");
      } catch (err: unknown) {
        setSubmitting(false);
        const authErr = err as AuthError;
        setStatus(authErr.message || "Registration failed. Check your connection and try again.");
      }
      return;
    }

    // Valid form, but no account backend exists: no account is created, no
    // email is sent, no session appears, and nothing is transmitted. The
    // short local beat exercises the real loading/disabled button states; the
    // terminal message is the honest one below.
    submitTimerRef.current = window.setTimeout(() => {
      setStatus(HONEST_SIGNUP_STATUS);
      setSubmitting(false);
      submitTimerRef.current = null;
      // Sensitive values are not kept after a completed submission.
      setPassword("");
      setConfirm("");
      setShowPassword(false);
      setShowConfirm(false);
    }, 400);
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
            Create account
          </li>
        </ol>

        <header className="signin__head">
          <h1 className="onboarding__title">Create your account</h1>
          <p className="onboarding__lead">
            Set up the details you will use to sign in to Nexora. Account
            creation is not connected in this build — you can walk through the
            form now, and nothing you type is submitted.
          </p>
        </header>

        <form className="signin__form" aria-label="Create account form" onSubmit={handleSubmit} noValidate>
          <div className="field">
            <label className="field__label" htmlFor="signup-name">
              Full name
            </label>
            <input
              ref={nameRef}
              id="signup-name"
              className="field__input field__input--ui"
              type="text"
              autoComplete="name"
              value={fullName}
              disabled={submitting}
              aria-invalid={errors.name ? true : undefined}
              aria-describedby={errors.name ? "signup-name-error" : undefined}
              onChange={(event) => {
                setFullName(event.target.value);
                if (errors.name) setErrors((current) => ({ ...current, name: undefined }));
              }}
            />
            {errors.name ? (
              <p id="signup-name-error" className="field__error" role="alert">
                {errors.name}
              </p>
            ) : null}
          </div>

          <div className="field">
            <label className="field__label" htmlFor="signup-email">
              Email address
            </label>
            <input
              ref={emailRef}
              id="signup-email"
              className="field__input field__input--ui"
              type="email"
              autoComplete="email"
              inputMode="email"
              spellCheck={false}
              value={email}
              disabled={submitting}
              aria-invalid={errors.email ? true : undefined}
              aria-describedby={errors.email ? "signup-email-error" : undefined}
              onChange={(event) => {
                setEmail(event.target.value);
                if (errors.email) setErrors((current) => ({ ...current, email: undefined }));
              }}
            />
            {errors.email ? (
              <p id="signup-email-error" className="field__error" role="alert">
                {errors.email}
              </p>
            ) : null}
          </div>

          <div className="field">
            <label className="field__label" htmlFor="signup-password">
              Password
            </label>
            <div className="field__row">
              <input
                ref={passwordRef}
                id="signup-password"
                className="field__input field__input--ui"
                type={showPassword ? "text" : "password"}
                autoComplete="new-password"
                value={password}
                disabled={submitting}
                aria-invalid={errors.password ? true : undefined}
                aria-describedby={errors.password ? "signup-password-error" : undefined}
                onChange={(event) => {
                  setPassword(event.target.value);
                  if (errors.password) setErrors((current) => ({ ...current, password: undefined }));
                }}
              />
              <button
                type="button"
                className="signin__visibility"
                aria-label={showPassword ? "Hide password" : "Show password"}
                aria-pressed={showPassword}
                aria-controls="signup-password"
                title={showPassword ? "Hide password" : "Show password"}
                onClick={() => setShowPassword((current) => !current)}
              >
                <Icon name={showPassword ? "eye-off" : "eye"} size={14} />
              </button>
            </div>
            {errors.password ? (
              <p id="signup-password-error" className="field__error" role="alert">
                {errors.password}
              </p>
            ) : (
              <p className="field__hint">Use at least {PASSWORD_MIN_LENGTH} characters.</p>
            )}
          </div>

          <div className="field">
            <label className="field__label" htmlFor="signup-confirm">
              Confirm password
            </label>
            <div className="field__row">
              <input
                ref={confirmRef}
                id="signup-confirm"
                className="field__input field__input--ui"
                type={showConfirm ? "text" : "password"}
                autoComplete="new-password"
                value={confirm}
                disabled={submitting}
                aria-invalid={errors.confirm ? true : undefined}
                aria-describedby={errors.confirm ? "signup-confirm-error" : undefined}
                onChange={(event) => {
                  setConfirm(event.target.value);
                  if (errors.confirm) setErrors((current) => ({ ...current, confirm: undefined }));
                }}
              />
              <button
                type="button"
                className="signin__visibility"
                aria-label={showConfirm ? "Hide password confirmation" : "Show password confirmation"}
                aria-pressed={showConfirm}
                aria-controls="signup-confirm"
                title={showConfirm ? "Hide password confirmation" : "Show password confirmation"}
                onClick={() => setShowConfirm((current) => !current)}
              >
                <Icon name={showConfirm ? "eye-off" : "eye"} size={14} />
              </button>
            </div>
            {errors.confirm ? (
              <p id="signup-confirm-error" className="field__error" role="alert">
                {errors.confirm}
              </p>
            ) : null}
          </div>

          <button type="submit" className="button button--primary signin__submit" disabled={submitting} aria-busy={submitting}>
            {submitting ? "Creating account…" : "Create account"}
          </button>
        </form>

        <div className="signin__status" role="status" aria-live="polite">
          {status ? (
            <div>
              <p className="signin__status-text">{status}</p>
              <p className="signin__status-next">
                <RouterLink to="verify" className="signin__link">
                  About email verification
                </RouterLink>
              </p>
            </div>
          ) : null}
        </div>

        <footer className="signin__footer">
          <div className="signin__links">
            <span className="signin__links-label">Already have an account?</span>
            <RouterLink to="signin" className="signin__link">
              Sign in
            </RouterLink>
          </div>
          <button type="button" className="button button--ghost button--sm" onClick={onBack}>
            Back to Sign In
          </button>
        </footer>
      </div>
    </div>
  );
}
