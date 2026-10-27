import { useId, useRef, useState } from "react";
import { Link } from "react-router-dom";

import ApertureMark from "../components/brand/ApertureMark";
import { Eyebrow } from "../components/ui";
import { ROUTES } from "../config/routes";
import { toFailure } from "./portal-api";
import { usePortalSession } from "./PortalSession";

import "./PortalAuth.css";

/**
 * The `/admin` sign-in page.
 *
 * Design notes, in the order they matter:
 *
 * - **It cannot pretend.** The only way this page reaches the dashboard is a
 *   `200` from `POST /api/portal/session` that also carried a session cookie. A
 *   failed request, a missing server, and a wrong password are three different
 *   states and none of them is "signed in".
 * - **It does not reveal whether an account exists.** Every credential failure
 *   shows the server's single generic message.
 * - **It is keyboard-complete.** Native form controls, a real `<form>`, the
 *   password toggle as a `type="button"` inside the field, and an error region
 *   that is announced (`role="alert"`) and referenced by `aria-describedby`.
 */

type Phase = "idle" | "submitting";

export default function PortalSignIn() {
  const { state, signIn } = usePortalSession();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [phase, setPhase] = useState<Phase>("idle");
  const [error, setError] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<{ email?: string; password?: string }>({});

  const emailId = useId();
  const passwordId = useId();
  const errorId = useId();
  const hintId = useId();
  const emailRef = useRef<HTMLInputElement>(null);
  const passwordRef = useRef<HTMLInputElement>(null);

  const unconfigured = state.status === "signed-out" && state.ownerProvisioned === false;

  /** Client-side checks are convenience only; the server validates again. */
  function validate(): boolean {
    const next: { email?: string; password?: string } = {};
    if (email.trim() === "") next.email = "Enter your email address.";
    if (password === "") next.password = "Enter your password.";
    setFieldErrors(next);
    if (next.email) {
      emailRef.current?.focus();
      return false;
    }
    if (next.password) {
      passwordRef.current?.focus();
      return false;
    }
    return true;
  }

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    // Guard against a double submit from a second click or Enter repeat.
    if (phase === "submitting") return;
    setError(null);
    if (!validate()) return;

    setPhase("submitting");
    try {
      await signIn(email.trim(), password);
      // The provider now holds a server-verified session; `PortalPage` swaps to
      // the dashboard. The password is cleared rather than kept in state.
      setPassword("");
    } catch (caught) {
      const failure = toFailure(caught);
      setError(failure.message);
      setPassword("");
      passwordRef.current?.focus();
    } finally {
      setPhase("idle");
    }
  }

  return (
    <div className="portal-auth">
      <div className="portal-auth__panel">
        <div className="portal-auth__identity">
          <ApertureMark size={26} className="portal-auth__mark" />
          <span className="portal-auth__wordmark">Nexora</span>
        </div>

        <Eyebrow>Developer Portal</Eyebrow>
        <h1 className="portal-auth__title">Sign in</h1>
        <p className="portal-auth__lead">
          Private access for authorised Nexora developers. This portal is not part of the public
          website, and there is no public registration.
        </p>

        {unconfigured ? (
          <div className="portal-auth__notice" role="status">
            <p className="portal-auth__notice-title">No owner account has been provisioned yet.</p>
            <p>
              The portal is running, but no account exists to sign in with. An owner account is
              created on the server with <code>node server/provision-owner.mjs</code>; it cannot be
              created from this page by design.
            </p>
          </div>
        ) : null}

        <form className="portal-auth__form" onSubmit={handleSubmit} noValidate>
          <div className="portal-field">
            <label className="portal-field__label" htmlFor={emailId}>
              Email address
            </label>
            <input
              id={emailId}
              ref={emailRef}
              className={`portal-field__input${fieldErrors.email ? " portal-field__input--invalid" : ""}`}
              type="email"
              name="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
              autoComplete="username"
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              required
              disabled={phase === "submitting" || unconfigured}
              aria-invalid={fieldErrors.email ? true : undefined}
              aria-describedby={fieldErrors.email ? `${errorId}-email` : undefined}
            />
            {fieldErrors.email ? (
              <p className="portal-field__error" id={`${errorId}-email`}>
                {fieldErrors.email}
              </p>
            ) : null}
          </div>

          <div className="portal-field">
            <label className="portal-field__label" htmlFor={passwordId}>
              Password
            </label>
            <div className="portal-field__control">
              <input
                id={passwordId}
                ref={passwordRef}
                className={`portal-field__input portal-field__input--with-action${
                  fieldErrors.password ? " portal-field__input--invalid" : ""
                }`}
                type={showPassword ? "text" : "password"}
                name="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                autoComplete="current-password"
                required
                disabled={phase === "submitting" || unconfigured}
                aria-invalid={fieldErrors.password ? true : undefined}
                aria-describedby={
                  [fieldErrors.password ? `${errorId}-password` : null, hintId].filter(Boolean).join(" ") ||
                  undefined
                }
              />
              <button
                type="button"
                className="portal-field__action"
                onClick={() => setShowPassword((current) => !current)}
                aria-pressed={showPassword}
                disabled={phase === "submitting" || unconfigured}
              >
                {showPassword ? "Hide" : "Show"}
              </button>
            </div>
            {fieldErrors.password ? (
              <p className="portal-field__error" id={`${errorId}-password`}>
                {fieldErrors.password}
              </p>
            ) : null}
          </div>

          <p className="portal-auth__hint" id={hintId}>
            Sessions end after a period of inactivity, and signing out revokes the session on the
            server.
          </p>

          <div className="portal-auth__status" aria-live="polite">
            {error !== null ? (
              <p className="portal-auth__error" role="alert" id={errorId}>
                {error}
              </p>
            ) : null}
          </div>

          <button
            type="submit"
            className="portal-auth__submit"
            disabled={phase === "submitting" || unconfigured}
            aria-busy={phase === "submitting"}
          >
            {phase === "submitting" ? "Signing in…" : "Sign in"}
          </button>
        </form>

        <p className="portal-auth__footnote">
          <Link to={ROUTES.home}>Return to the Nexora website</Link>
        </p>
      </div>
    </div>
  );
}
