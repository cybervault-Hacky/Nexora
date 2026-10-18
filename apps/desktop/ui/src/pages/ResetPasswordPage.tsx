import { useEffect, useRef, useState, type FormEvent } from "react";
import { ApertureMark } from "../components/ApertureMark";
import { Icon } from "../components/Icon";
import {
  PASSWORD_MIN_LENGTH,
  validateNewPassword,
  validatePasswordConfirmation,
} from "../lib/formValidation";

interface ResetPasswordPageProps {
  /** Returns to the sign-in page. */
  onBack: () => void;
}

interface FieldErrors {
  password?: string;
  confirm?: string;
}

const HONEST_RESET_STATUS =
  "Password reset is not connected yet. No password has been changed.";

/**
 * The password-reset interface — the page future recovery emails will link to.
 *
 * Route decision (Phase 22): implemented because the authentication backend's
 * recovery emails need a frontend target, and having the page now makes the
 * future flow concrete. It is reachable by direct route today. It does not
 * read, accept, or validate reset tokens — that requires the future backend —
 * and it never claims a password was updated.
 */
export function ResetPasswordPage({ onBack }: ResetPasswordPageProps) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirm, setShowConfirm] = useState(false);
  const [errors, setErrors] = useState<FieldErrors>({});
  const [submitting, setSubmitting] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

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

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (submitting) return;

    const nextErrors: FieldErrors = {};
    const passwordError = validateNewPassword(password);
    const confirmError = validatePasswordConfirmation(password, confirm);
    if (passwordError) nextErrors.password = passwordError;
    if (confirmError) nextErrors.confirm = confirmError;

    setErrors(nextErrors);
    setStatus(null);

    if (nextErrors.password || nextErrors.confirm) {
      if (nextErrors.password) passwordRef.current?.focus();
      else confirmRef.current?.focus();
      return;
    }

    // Valid form, but no reset backend exists: no token is involved and no
    // password is changed. Sensitive values are cleared once this submission
    // completes; validation failures keep everything you typed.
    setSubmitting(true);
    submitTimerRef.current = window.setTimeout(() => {
      setStatus(HONEST_RESET_STATUS);
      setSubmitting(false);
      submitTimerRef.current = null;
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

        <header className="signin__head">
          <h1 className="onboarding__title">Choose a new password</h1>
          <p className="onboarding__lead">
            Set a new password for your account. Password resetting requires the
            future authentication backend — nothing is changed in this build.
          </p>
        </header>

        <form className="signin__form" aria-label="Password reset form" onSubmit={handleSubmit} noValidate>
          <div className="field">
            <label className="field__label" htmlFor="reset-password">
              New password
            </label>
            <div className="field__row">
              <input
                ref={passwordRef}
                id="reset-password"
                className="field__input field__input--ui"
                type={showPassword ? "text" : "password"}
                autoComplete="new-password"
                value={password}
                disabled={submitting}
                aria-invalid={errors.password ? true : undefined}
                aria-describedby={errors.password ? "reset-password-error" : undefined}
                onChange={(event) => {
                  setPassword(event.target.value);
                  if (errors.password) setErrors((current) => ({ ...current, password: undefined }));
                }}
              />
              <button
                type="button"
                className="signin__visibility"
                aria-label={showPassword ? "Hide new password" : "Show new password"}
                aria-pressed={showPassword}
                aria-controls="reset-password"
                title={showPassword ? "Hide new password" : "Show new password"}
                onClick={() => setShowPassword((current) => !current)}
              >
                <Icon name={showPassword ? "eye-off" : "eye"} size={14} />
              </button>
            </div>
            {errors.password ? (
              <p id="reset-password-error" className="field__error" role="alert">
                {errors.password}
              </p>
            ) : (
              <p className="field__hint">Use at least {PASSWORD_MIN_LENGTH} characters.</p>
            )}
          </div>

          <div className="field">
            <label className="field__label" htmlFor="reset-confirm">
              Confirm new password
            </label>
            <div className="field__row">
              <input
                ref={confirmRef}
                id="reset-confirm"
                className="field__input field__input--ui"
                type={showConfirm ? "text" : "password"}
                autoComplete="new-password"
                value={confirm}
                disabled={submitting}
                aria-invalid={errors.confirm ? true : undefined}
                aria-describedby={errors.confirm ? "reset-confirm-error" : undefined}
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
                aria-controls="reset-confirm"
                title={showConfirm ? "Hide password confirmation" : "Show password confirmation"}
                onClick={() => setShowConfirm((current) => !current)}
              >
                <Icon name={showConfirm ? "eye-off" : "eye"} size={14} />
              </button>
            </div>
            {errors.confirm ? (
              <p id="reset-confirm-error" className="field__error" role="alert">
                {errors.confirm}
              </p>
            ) : null}
          </div>

          <button type="submit" className="button button--primary signin__submit" disabled={submitting} aria-busy={submitting}>
            {submitting ? "Resetting…" : "Reset password"}
          </button>
        </form>

        <div className="signin__status" role="status" aria-live="polite">
          {status ? <p className="signin__status-text">{status}</p> : null}
        </div>

        <div className="onboarding__slot" role="note" aria-label="About reset links">
          <Icon name="shield" size={16} />
          <div>
            <strong>Reset links will carry a token.</strong>
            <p>
              When password recovery is connected, its emails will link here
              with a token for the authentication backend to validate. This
              page does not accept or check tokens.
            </p>
          </div>
        </div>

        <div className="onboarding__actions">
          <button type="button" className="button button--ghost" onClick={onBack}>
            Back to Sign In
          </button>
        </div>
      </div>
    </div>
  );
}
