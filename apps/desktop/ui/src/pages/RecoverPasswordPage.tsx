import { useEffect, useRef, useState, type FormEvent } from "react";
import { ApertureMark } from "../components/ApertureMark";
import { RouterLink } from "../components/RouterLink";
import { validateEmail } from "../lib/formValidation";

interface RecoverPasswordPageProps {
  /** Returns to the sign-in page. */
  onBack: () => void;
}

const HONEST_RECOVERY_STATUS =
  "Password recovery is not connected yet. No reset email has been sent.";

/**
 * The Phase 22 password-recovery interface.
 *
 * The form validates and submits locally only: the email is never transmitted,
 * no recovery request is made, and no message ever claims that an account
 * exists or that an email was sent.
 */
export function RecoverPasswordPage({ onBack }: RecoverPasswordPageProps) {
  const [email, setEmail] = useState("");
  const [error, setError] = useState<string | undefined>(undefined);
  const [submitting, setSubmitting] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const emailRef = useRef<HTMLInputElement>(null);
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

    const emailError = validateEmail(email);
    setError(emailError);
    setStatus(null);

    if (emailError) {
      emailRef.current?.focus();
      return;
    }

    // Valid email, but recovery is not connected: nothing is transmitted and
    // no email is sent. The short local beat exercises the loading/disabled
    // button states; the terminal message is the honest one below.
    setSubmitting(true);
    submitTimerRef.current = window.setTimeout(() => {
      setStatus(HONEST_RECOVERY_STATUS);
      setSubmitting(false);
      submitTimerRef.current = null;
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
          <h1 className="onboarding__title">Reset your password</h1>
          <p className="onboarding__lead">
            Enter your email address to request a password reset. Recovery is
            not connected in this build — no request leaves this window, and no
            reset email can be sent yet.
          </p>
        </header>

        <form className="signin__form" aria-label="Password recovery form" onSubmit={handleSubmit} noValidate>
          <div className="field">
            <label className="field__label" htmlFor="recover-email">
              Email address
            </label>
            <input
              ref={emailRef}
              id="recover-email"
              className="field__input field__input--ui"
              type="email"
              autoComplete="email"
              inputMode="email"
              spellCheck={false}
              value={email}
              disabled={submitting}
              aria-invalid={error ? true : undefined}
              aria-describedby={error ? "recover-email-error" : undefined}
              onChange={(event) => {
                setEmail(event.target.value);
                if (error) setError(undefined);
              }}
            />
            {error ? (
              <p id="recover-email-error" className="field__error" role="alert">
                {error}
              </p>
            ) : null}
          </div>

          <button type="submit" className="button button--primary signin__submit" disabled={submitting} aria-busy={submitting}>
            {submitting ? "Continuing…" : "Continue"}
          </button>
        </form>

        <div className="signin__status" role="status" aria-live="polite">
          {status ? (
            <div>
              <p className="signin__status-text">{status}</p>
              <p className="signin__status-next">
                <RouterLink to="reset" className="signin__link">
                  About password reset
                </RouterLink>
              </p>
            </div>
          ) : null}
        </div>

        <footer className="signin__footer">
          <div className="signin__links">
            <span className="signin__links-label">Remembered your password?</span>
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
