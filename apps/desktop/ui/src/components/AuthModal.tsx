import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
  type RefObject,
} from "react";
import { ApertureMark } from "./ApertureMark";
import { GoogleMark } from "./GoogleMark";
import { Icon } from "./Icon";
import {
  getDeveloperDiagnostics,
  type AuthStatusResponse,
  type AuthUser,
  type LoginPayload,
  type RegisterPayload,
} from "../lib/auth";
import { authErrorCode, safeAuthErrorMessage, safeServiceMessage } from "../lib/authMessages";
import { isDesktopShell } from "../lib/ipc";
import {
  PASSWORD_MIN_LENGTH,
  validateEmail,
  validateFullName,
  validateNewPassword,
  validatePasswordConfirmation,
} from "../lib/formValidation";

const TOKEN_PATTERN = /^[a-f0-9]{64}$/i;

type AuthField = "name" | "email" | "password" | "confirmPassword" | "token";

function validateToken(value: string, label: string): string | undefined {
  const token = value.trim();
  if (token.length === 0) return `Enter the ${label} from your email.`;
  if (!TOKEN_PATTERN.test(token)) return `Enter the 64-character ${label} from your email.`;
  return undefined;
}

function serviceConfigurationState(value: unknown): boolean | null {
  if (typeof value !== "string") return null;
  const url = value.trim().toLowerCase();
  if (!url) return false;
  if (url === "unconfigured" || url === "invalid_backend_url") return false;
  return true;
}

function responseStatus(response: AuthStatusResponse): string {
  return typeof response.status === "string" ? response.status.toLowerCase() : "";
}

function responseDelivery(response: AuthStatusResponse): string {
  return typeof response.emailDelivery === "string" ? response.emailDelivery.toLowerCase() : "";
}

function focusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll<HTMLElement>(
      'button:not([disabled]):not([tabindex="-1"]), a[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((element) => !element.hasAttribute("hidden") && element.getAttribute("aria-hidden") !== "true");
}

export type AuthModalMode = "signin" | "signup" | "recover" | "reset" | "verify";

interface AuthModalProps {
  open: boolean;
  initialMode?: AuthModalMode;
  initialToken?: string;
  reason?: string | null;
  preservedPrompt?: string;
  returnFocusTarget?: HTMLElement | null;
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
  initialToken = "",
  reason = null,
  preservedPrompt,
  returnFocusTarget,
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
  const subtitleId = useId();
  const reasonId = useId();
  const errorId = useId();
  const noticeId = useId();
  const signInTabId = useId();
  const signUpTabId = useId();
  const signInPanelId = useId();
  const signUpPanelId = useId();
  const googleHelpId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);
  const emailRef = useRef<HTMLInputElement>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  const passwordRef = useRef<HTMLInputElement>(null);
  const confirmPasswordRef = useRef<HTMLInputElement>(null);
  const tokenRef = useRef<HTMLInputElement>(null);
  const openRef = useRef(open);
  const cancelGoogleRef = useRef(onCancelGoogle);
  const requestIdRef = useRef(0);
  const requestInFlightRef = useRef(false);
  const googleFlowRef = useRef(false);

  // Keep these guards current even before React commits state updates.
  openRef.current = open;
  cancelGoogleRef.current = onCancelGoogle;

  const [mode, setMode] = useState<AuthModalMode>(initialMode);
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [googleBusy, setGoogleBusy] = useState(false);
  const [authServiceConfigured, setAuthServiceConfigured] = useState<boolean | null>(null);
  const [googleOauthConfigured, setGoogleOauthConfigured] = useState<boolean | null>(null);
  const [authConfigurationChecking, setAuthConfigurationChecking] = useState(false);
  const [googleUnavailable, setGoogleUnavailable] = useState(false);
  const [googleUnavailableMessage, setGoogleUnavailableMessage] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [errorField, setErrorField] = useState<AuthField | null>(null);

  const fieldRefs: Record<AuthField, RefObject<HTMLInputElement | null>> = {
    name: nameRef,
    email: emailRef,
    password: passwordRef,
    confirmPassword: confirmPasswordRef,
    token: tokenRef,
  };

  useLayoutEffect(() => {
    if (!open) return;

    const activeElement = document.activeElement;
    const fallbackTarget = activeElement instanceof HTMLElement ? activeElement : null;
    const restoreTarget =
      returnFocusTarget?.isConnected ? returnFocusTarget : fallbackTarget;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    const target = dialogRef.current?.querySelector<HTMLElement>("[data-auth-autofocus]");
    (target ?? dialogRef.current)?.focus();

    return () => {
      document.body.style.overflow = previousOverflow;
      if (restoreTarget?.isConnected) {
        restoreTarget.focus();
        return;
      }
      // A successful login replaces the signed-out trigger with the account entry.
      // Keep keyboard focus in the current workspace when that original node is gone.
      document.querySelector<HTMLElement>(".workspace")?.focus();
    };
  }, [open, returnFocusTarget]);

  useLayoutEffect(() => {
    if (!open) return;
    const dialog = dialogRef.current;
    if (!dialog || dialog.contains(document.activeElement)) return;
    const target = dialog.querySelector<HTMLElement>("[data-auth-autofocus]");
    (target ?? dialog).focus();
  }, [mode, open]);

  useEffect(() => {
    if (!open) return;

    setMode(initialMode);
    setName("");
    setEmail("");
    setError(null);
    setErrorField(null);
    setNotice(null);
    setPassword("");
    setConfirmPassword("");
    setToken(initialToken.trim());
    setBusy(false);
    setGoogleBusy(false);
    setAuthServiceConfigured(null);
    setGoogleOauthConfigured(null);
    setGoogleUnavailable(false);
    setGoogleUnavailableMessage("");
    requestInFlightRef.current = false;
    googleFlowRef.current = false;
    requestIdRef.current += 1;

    if (!isDesktopShell()) {
      setAuthServiceConfigured(false);
      setAuthConfigurationChecking(false);
      return;
    }

    let cancelled = false;
    setAuthConfigurationChecking(true);
    void getDeveloperDiagnostics()
      .then((diagnostics) => {
        if (cancelled) return;
        if (diagnostics && typeof diagnostics === "object" && "authServiceUrl" in diagnostics) {
          setAuthServiceConfigured(serviceConfigurationState(diagnostics.authServiceUrl));
          setGoogleOauthConfigured(
            typeof diagnostics.googleOauthConfigured === "boolean"
              ? diagnostics.googleOauthConfigured
              : null,
          );
        } else {
          // Older or mocked desktop diagnostics may not describe configuration.
          setAuthServiceConfigured(null);
          setGoogleOauthConfigured(null);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setAuthServiceConfigured(null);
          setGoogleOauthConfigured(null);
        }
      })
      .finally(() => {
        if (!cancelled) setAuthConfigurationChecking(false);
      });

    return () => {
      cancelled = true;
    };
  }, [open, initialMode, initialToken]);

  useEffect(() => {
    if (open) return;
    if (googleFlowRef.current) {
      googleFlowRef.current = false;
      void onCancelGoogle().catch(() => {});
    }
    requestIdRef.current += 1;
    requestInFlightRef.current = false;
    setBusy(false);
    setGoogleBusy(false);
    setName("");
    setEmail("");
    setPassword("");
    setConfirmPassword("");
    setToken("");
  }, [open, onCancelGoogle]);

  useEffect(
    () => () => {
      openRef.current = false;
      requestIdRef.current += 1;
      requestInFlightRef.current = false;
      if (googleFlowRef.current) {
        googleFlowRef.current = false;
        void cancelGoogleRef.current().catch(() => {});
      }
    },
    [],
  );

  function beginRequest(): number | null {
    if (!openRef.current || requestInFlightRef.current) return null;
    requestInFlightRef.current = true;
    const requestId = ++requestIdRef.current;
    setBusy(true);
    setError(null);
    setErrorField(null);
    setNotice(null);
    return requestId;
  }

  function requestIsCurrent(requestId: number): boolean {
    return openRef.current && requestIdRef.current === requestId;
  }

  function finishRequest(requestId: number): void {
    if (requestIdRef.current !== requestId) return;
    requestInFlightRef.current = false;
    setBusy(false);
  }

  function clearError(): void {
    setError(null);
    setErrorField(null);
  }

  function showValidationError(message: string, field: AuthField): void {
    setError(message);
    setErrorField(field);
    setNotice(null);
    fieldRefs[field].current?.focus();
  }

  function switchMode(next: AuthModalMode): void {
    if (requestInFlightRef.current || next === mode) return;
    setMode(next);
    setError(null);
    setErrorField(null);
    setNotice(null);
    // Secrets are not carried between unrelated auth operations.
    setPassword("");
    setConfirmPassword("");
    if (next !== "verify" && next !== "reset") setToken("");
  }

  function closeModal(): void {
    const shouldCancelGoogle = googleFlowRef.current;
    requestIdRef.current += 1;
    requestInFlightRef.current = false;
    googleFlowRef.current = false;
    openRef.current = false;
    setBusy(false);
    setGoogleBusy(false);
    setName("");
    setEmail("");
    setPassword("");
    setConfirmPassword("");
    setToken("");
    if (shouldCancelGoogle) void onCancelGoogle().catch(() => {});
    onClose();
  }

  function verifyServiceIsConfigured(): boolean {
    if (authServiceConfigured !== false) return true;
    setError(
      "Account authentication isn't configured in this build. Local chat and workspace tools remain available.",
    );
    setErrorField(null);
    return false;
  }

  async function handleSignIn(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (requestInFlightRef.current) return;
    clearError();
    setNotice(null);

    const emailError = validateEmail(email);
    if (emailError) {
      showValidationError(emailError, "email");
      return;
    }
    if (!password) {
      showValidationError("Enter your password.", "password");
      return;
    }
    if (!verifyServiceIsConfigured()) return;

    const requestId = beginRequest();
    if (requestId === null) return;
    try {
      await onLogin({ email: email.trim(), password });
      if (!requestIsCurrent(requestId)) return;
      setPassword("");
      closeModal();
    } catch (cause) {
      if (!requestIsCurrent(requestId)) return;
      const code = authErrorCode(cause);
      const message = safeAuthErrorMessage(cause, "signin", [email, password, token]);
      setPassword("");
      setErrorField(null);
      if (code === "email_unverified" || code === "email_not_verified") {
        setNotice(message);
        setMode("verify");
        setToken("");
      } else {
        setError(message);
      }
    } finally {
      finishRequest(requestId);
    }
  }

  async function handleSignUp(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (requestInFlightRef.current) return;
    clearError();
    setNotice(null);

    const nameError = validateFullName(name);
    if (nameError) {
      showValidationError(nameError, "name");
      return;
    }
    const emailError = validateEmail(email);
    if (emailError) {
      showValidationError(emailError, "email");
      return;
    }
    const passwordError = validateNewPassword(password);
    if (passwordError) {
      showValidationError(passwordError, "password");
      return;
    }
    const confirmationError = validatePasswordConfirmation(password, confirmPassword);
    if (confirmationError) {
      showValidationError(confirmationError, "confirmPassword");
      return;
    }
    if (!verifyServiceIsConfigured()) return;

    const requestId = beginRequest();
    if (requestId === null) return;
    try {
      const response = await onRegister({
        name: name.trim(),
        email: email.trim(),
        password,
      });
      if (!requestIsCurrent(requestId)) return;
      const responseMessage = safeServiceMessage(response.message, [email, password, token]);
      const delivery = responseDelivery(response);
      const accepted = ["verification_pending", "accepted", "registered"].includes(
        responseStatus(response),
      );
      setPassword("");
      setConfirmPassword("");
      setNotice(
        responseMessage ??
          (delivery === "unconfigured"
            ? "The registration request was accepted, but email delivery is not configured. No verification email was sent."
            : "The registration request was accepted, but the service did not provide a confirmation message."),
      );
      if (accepted) {
        setMode("verify");
      } else {
        setErrorField(null);
      }
    } catch (cause) {
      if (!requestIsCurrent(requestId)) return;
      setError(safeAuthErrorMessage(cause, "signup", [name, email, password, confirmPassword, token]));
      setErrorField(null);
    } finally {
      finishRequest(requestId);
    }
  }

  async function handleVerify(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (requestInFlightRef.current) return;
    clearError();
    setNotice(null);

    const tokenError = validateToken(token, "verification code");
    if (tokenError) {
      showValidationError(tokenError, "token");
      return;
    }
    if (!verifyServiceIsConfigured()) return;

    const requestId = beginRequest();
    if (requestId === null) return;
    try {
      const response = await onVerifyEmail(token.trim());
      if (!requestIsCurrent(requestId)) return;
      const confirmed = responseStatus(response) === "verified";
      const responseMessage = safeServiceMessage(response.message, [email, token, password]);
      setToken("");
      setNotice(
        responseMessage ??
          (confirmed
            ? "The service confirmed your email address. You can now sign in."
            : "The service accepted the request but did not confirm email verification."),
      );
      if (confirmed) setMode("signin");
    } catch (cause) {
      if (!requestIsCurrent(requestId)) return;
      setError(safeAuthErrorMessage(cause, "verify", [email, token, password]));
      setErrorField(null);
    } finally {
      finishRequest(requestId);
    }
  }

  async function handleResendVerification(): Promise<void> {
    if (requestInFlightRef.current) return;
    clearError();

    const emailError = validateEmail(email);
    if (emailError) {
      showValidationError("Enter a valid email address to request another verification message.", "email");
      return;
    }
    if (!verifyServiceIsConfigured()) return;

    const requestId = beginRequest();
    if (requestId === null) return;
    try {
      const response = await onResendVerification(email.trim());
      if (!requestIsCurrent(requestId)) return;
      const message = safeServiceMessage(response.message, [email, password, token]);
      setNotice(
        message ??
          (responseDelivery(response) === "unconfigured"
            ? "No verification email was sent because email delivery is not configured."
            : "The verification request was accepted. The service did not confirm email delivery."),
      );
    } catch (cause) {
      if (!requestIsCurrent(requestId)) return;
      setError(safeAuthErrorMessage(cause, "verify", [email, password, token]));
    } finally {
      finishRequest(requestId);
    }
  }

  async function handleRecover(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (requestInFlightRef.current) return;
    clearError();
    setNotice(null);

    const emailError = validateEmail(email);
    if (emailError) {
      showValidationError(emailError, "email");
      return;
    }
    if (!verifyServiceIsConfigured()) return;

    const requestId = beginRequest();
    if (requestId === null) return;
    try {
      const response = await onRecoverPassword(email.trim());
      if (!requestIsCurrent(requestId)) return;
      const delivery = responseDelivery(response);
      const responseMessage = safeServiceMessage(response.message, [email, password, token]);
      setNotice(
        responseMessage ??
          (delivery === "unconfigured"
            ? "The request was accepted, but no email was sent because email delivery is not configured."
            : "The recovery request was accepted. The service did not provide delivery details."),
      );
      if (delivery !== "unconfigured" && ["accepted", "sent", "requested"].includes(responseStatus(response))) {
        setMode("reset");
      }
    } catch (cause) {
      if (!requestIsCurrent(requestId)) return;
      setError(safeAuthErrorMessage(cause, "recover", [email, password, token]));
    } finally {
      finishRequest(requestId);
    }
  }

  async function handleReset(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (requestInFlightRef.current) return;
    clearError();
    setNotice(null);

    const tokenError = validateToken(token, "reset token");
    if (tokenError) {
      showValidationError(tokenError, "token");
      return;
    }
    const passwordError = validateNewPassword(password);
    if (passwordError) {
      showValidationError(passwordError, "password");
      return;
    }
    if (!verifyServiceIsConfigured()) return;

    const requestId = beginRequest();
    if (requestId === null) return;
    try {
      const response = await onResetPassword(token.trim(), password);
      if (!requestIsCurrent(requestId)) return;
      const confirmed = ["reset", "success"].includes(responseStatus(response));
      const responseMessage = safeServiceMessage(response.message, [email, password, token]);
      setPassword("");
      setToken("");
      setNotice(
        responseMessage ??
          (confirmed
            ? "The service confirmed the password reset. Sign in with the new password."
            : "The service accepted the request but did not confirm the password reset."),
      );
      if (confirmed) setMode("signin");
    } catch (cause) {
      if (!requestIsCurrent(requestId)) return;
      setError(safeAuthErrorMessage(cause, "reset", [email, password, token]));
      setErrorField(null);
    } finally {
      finishRequest(requestId);
    }
  }

  async function handleGoogle(): Promise<void> {
    if (requestInFlightRef.current || !isDesktopShell()) return;
    clearError();
    setNotice(null);
    if (authServiceConfigured === false) {
      setGoogleUnavailableMessage("Google sign-in is unavailable because account authentication is not configured in this build.");
      setGoogleUnavailable(true);
      return;
    }
    if (googleOauthConfigured === false) {
      setGoogleUnavailableMessage("Google sign-in is not configured in this desktop build. Local chat and email sign-in remain available.");
      setGoogleUnavailable(true);
      return;
    }

    const requestId = beginRequest();
    if (requestId === null) return;
    googleFlowRef.current = true;
    setGoogleBusy(true);
    try {
      await onGoogleSignIn();
      if (!requestIsCurrent(requestId)) return;
      googleFlowRef.current = false;
      closeModal();
    } catch (cause) {
      if (!requestIsCurrent(requestId)) return;
      const code = authErrorCode(cause);
      const message = safeAuthErrorMessage(cause, "google", [email, password, token]);
      googleFlowRef.current = false;
      setGoogleBusy(false);
      if (code === "dependency_unavailable" || code === "backend_unconfigured") {
        setGoogleUnavailable(true);
        setGoogleUnavailableMessage(message);
      } else if (code !== "cancelled") {
        setError(message);
      }
    } finally {
      finishRequest(requestId);
      if (requestIdRef.current === requestId) setGoogleBusy(false);
    }
  }

  function cancelGoogle(): void {
    if (!googleFlowRef.current) return;
    requestIdRef.current += 1;
    requestInFlightRef.current = false;
    googleFlowRef.current = false;
    setBusy(false);
    setGoogleBusy(false);
    void onCancelGoogle().catch(() => {});
  }

  useEffect(() => {
    if (!open) return;

    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        if (googleFlowRef.current) cancelGoogle();
        else closeModal();
        return;
      }
      if (event.key !== "Tab") return;

      const dialog = dialogRef.current;
      if (!dialog) return;
      const elements = focusableElements(dialog);
      if (elements.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      const first = elements[0]!;
      const last = elements[elements.length - 1]!;
      const active = document.activeElement;
      if (event.shiftKey && (active === first || !dialog.contains(active))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (active === last || !dialog.contains(active))) {
        event.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [open]);

  if (!open) return null;

  const googleUnavailableReason = !isDesktopShell()
    ? "Google sign-in is available only in the Nexora desktop app."
    : authServiceConfigured === false
      ? "Account authentication is not configured in this build."
      : googleOauthConfigured === false
        ? "Google sign-in is not configured in this desktop build. Local chat and email sign-in remain available."
        : googleUnavailable
          ? googleUnavailableMessage || "Google sign-in is unavailable for this account service."
          : authConfigurationChecking
            ? "Checking Google sign-in availability…"
            : null;
  const googleDisabled = Boolean(googleUnavailableReason) || busy || googleBusy;
  const authDescriptionIds = [
    subtitleId,
    reason ? reasonId : null,
    error ? errorId : null,
    notice ? noticeId : null,
  ]
    .filter(Boolean)
    .join(" ");

  function onAuthTabKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const next: AuthModalMode = mode === "signin" ? "signup" : "signin";
    switchMode(next);
    const tabMode = next === "signin" ? "signin" : "signup";
    window.requestAnimationFrame(() => {
      dialogRef.current?.querySelector<HTMLElement>(`[data-auth-tab="${tabMode}"]`)?.focus();
    });
  }

  const errorProps = (field: AuthField) => ({
    "aria-invalid": error && errorField === field ? true : undefined,
    "aria-describedby": error && errorField === field ? errorId : undefined,
  });

  return (
    <div
      className="auth-modal-backdrop"
      role="presentation"
      data-testid="auth-modal-backdrop"
      onClick={(event) => {
        if (event.target === event.currentTarget) closeModal();
      }}
    >
      <div
        ref={dialogRef}
        className="auth-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={authDescriptionIds || undefined}
        aria-busy={busy || googleBusy}
        tabIndex={-1}
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
            aria-label="Close authentication dialog"
            onClick={closeModal}
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

        <p id={subtitleId} className="auth-modal__subtitle">
          Local AI inference and conversation history stay on your machine. Sign in is optional for local chat.
        </p>

        {reason ? (
          <div id={reasonId} className="auth-modal__reason" role="note" data-testid="auth-modal-reason">
            <Icon name="shield" size={14} />
            <span>{reason}</span>
          </div>
        ) : null}

        {preservedPrompt?.trim() ? (
          <div className="auth-modal__preserved" data-testid="auth-preserved-prompt">
            <Icon name="message" size={14} />
            <span>Your current draft will be preserved when you close or complete sign-in.</span>
          </div>
        ) : null}

        {authServiceConfigured === false ? (
          <div className="auth-modal__unconfigured" role="status" data-testid="auth-backend-note">
            <span>
              Account authentication is not configured in this build. Close this dialog anytime to continue chatting locally.
            </span>
          </div>
        ) : null}

        {(mode === "signin" || mode === "signup") && (
          <div
            className="auth-modal__tabs"
            role="tablist"
            aria-label="Authentication mode"
            onKeyDown={onAuthTabKeyDown}
          >
            <button
              id={signInTabId}
              data-auth-tab="signin"
              type="button"
              role="tab"
              aria-selected={mode === "signin"}
              aria-controls={mode === "signin" ? signInPanelId : undefined}
              tabIndex={mode === "signin" ? 0 : -1}
              className={`auth-modal__tab${mode === "signin" ? " auth-modal__tab--active" : ""}`}
              onClick={() => switchMode("signin")}
              disabled={busy || googleBusy}
            >
              Sign in
            </button>
            <button
              id={signUpTabId}
              data-auth-tab="signup"
              type="button"
              role="tab"
              aria-selected={mode === "signup"}
              aria-controls={mode === "signup" ? signUpPanelId : undefined}
              tabIndex={mode === "signup" ? 0 : -1}
              className={`auth-modal__tab${mode === "signup" ? " auth-modal__tab--active" : ""}`}
              onClick={() => switchMode("signup")}
              disabled={busy || googleBusy}
            >
              Create account
            </button>
          </div>
        )}

        {error ? (
          <div id={errorId} className="auth-modal__alert auth-modal__alert--error" role="alert">
            <Icon name="alert" size={15} />
            <span>{error}</span>
          </div>
        ) : null}

        {notice ? (
          <div id={noticeId} className="auth-modal__alert auth-modal__alert--info" role="status">
            <Icon name="check" size={15} />
            <span>{notice}</span>
          </div>
        ) : null}

        {(mode === "signin" || mode === "signup") && (
          <>
            <button
              type="button"
              className="auth-modal__google-btn"
              onClick={() => void handleGoogle()}
              disabled={googleDisabled}
              aria-describedby={googleUnavailableReason ? googleHelpId : undefined}
              title={googleUnavailableReason ?? undefined}
            >
              <GoogleMark size={16} />
              <span>{googleBusy ? "Waiting for Google sign-in…" : "Continue with Google"}</span>
            </button>
            {googleUnavailableReason ? (
              <p id={googleHelpId} className="auth-modal__unavailable" role={googleUnavailable ? "status" : "note"}>
                {googleUnavailableReason}
              </p>
            ) : null}

            <div className="auth-modal__divider" aria-hidden="true">
              <span>or continue with email</span>
            </div>
          </>
        )}

        {mode === "signin" ? (
          <form
            id={signInPanelId}
            role="tabpanel"
            aria-labelledby={signInTabId}
            className="auth-modal__form"
            onSubmit={(event) => void handleSignIn(event)}
            noValidate
          >
            <label className="auth-modal__field">
              <span>Email address</span>
              <input
                ref={emailRef}
                className="field__input field__input--ui"
                type="email"
                autoComplete="email"
                autoCapitalize="none"
                inputMode="email"
                spellCheck={false}
                value={email}
                onChange={(event) => {
                  setEmail(event.target.value);
                  if (errorField === "email") clearError();
                }}
                placeholder="you@example.com"
                disabled={busy || googleBusy}
                required
                data-auth-autofocus
                {...errorProps("email")}
              />
            </label>

            <label className="auth-modal__field">
              <span>Password</span>
              <input
                ref={passwordRef}
                className="field__input field__input--ui"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(event) => {
                  setPassword(event.target.value);
                  if (errorField === "password") clearError();
                }}
                placeholder="Enter your password"
                disabled={busy || googleBusy}
                required
                {...errorProps("password")}
              />
            </label>

            <button
              type="submit"
              className="button button--primary auth-modal__submit"
              disabled={busy || googleBusy || authServiceConfigured === false}
              aria-busy={busy}
            >
              {busy ? "Signing in…" : "Sign in"}
            </button>

            <div className="auth-modal__footer-links">
              <button type="button" className="auth-modal__link" onClick={() => switchMode("recover")} disabled={busy}>
                Forgot password?
              </button>
              <button type="button" className="auth-modal__link" onClick={() => switchMode("verify")} disabled={busy}>
                Have a verification code?
              </button>
            </div>
          </form>
        ) : null}

        {mode === "signup" ? (
          <form
            id={signUpPanelId}
            role="tabpanel"
            aria-labelledby={signUpTabId}
            className="auth-modal__form"
            onSubmit={(event) => void handleSignUp(event)}
            noValidate
          >
            <label className="auth-modal__field">
              <span>Name</span>
              <input
                ref={nameRef}
                className="field__input field__input--ui"
                type="text"
                autoComplete="name"
                value={name}
                onChange={(event) => {
                  setName(event.target.value);
                  if (errorField === "name") clearError();
                }}
                placeholder="Ada Lovelace"
                disabled={busy || googleBusy}
                required
                data-auth-autofocus
                {...errorProps("name")}
              />
            </label>

            <label className="auth-modal__field">
              <span>Email address</span>
              <input
                ref={emailRef}
                className="field__input field__input--ui"
                type="email"
                autoComplete="email"
                autoCapitalize="none"
                inputMode="email"
                spellCheck={false}
                value={email}
                onChange={(event) => {
                  setEmail(event.target.value);
                  if (errorField === "email") clearError();
                }}
                placeholder="you@example.com"
                disabled={busy || googleBusy}
                required
                {...errorProps("email")}
              />
            </label>

            <label className="auth-modal__field">
              <span>Password</span>
              <input
                ref={passwordRef}
                className="field__input field__input--ui"
                type="password"
                autoComplete="new-password"
                value={password}
                onChange={(event) => {
                  setPassword(event.target.value);
                  if (errorField === "password") clearError();
                }}
                placeholder={`At least ${PASSWORD_MIN_LENGTH} characters`}
                disabled={busy || googleBusy}
                required
                {...errorProps("password")}
              />
            </label>

            <label className="auth-modal__field">
              <span>Confirm password</span>
              <input
                ref={confirmPasswordRef}
                className="field__input field__input--ui"
                type="password"
                autoComplete="new-password"
                value={confirmPassword}
                onChange={(event) => {
                  setConfirmPassword(event.target.value);
                  if (errorField === "confirmPassword") clearError();
                }}
                placeholder="Re-enter your password"
                disabled={busy || googleBusy}
                required
                {...errorProps("confirmPassword")}
              />
            </label>

            <button
              type="submit"
              className="button button--primary auth-modal__submit"
              disabled={busy || googleBusy || authServiceConfigured === false}
              aria-busy={busy}
            >
              {busy ? "Creating account…" : "Create account"}
            </button>
          </form>
        ) : null}

        {mode === "verify" ? (
          <form className="auth-modal__form" onSubmit={(event) => void handleVerify(event)} noValidate>
            <label className="auth-modal__field">
              <span>Email address</span>
              <input
                ref={emailRef}
                className="field__input field__input--ui"
                type="email"
                autoComplete="email"
                autoCapitalize="none"
                inputMode="email"
                value={email}
                onChange={(event) => {
                  setEmail(event.target.value);
                  if (errorField === "email") clearError();
                }}
                placeholder="you@example.com"
                disabled={busy}
                {...errorProps("email")}
              />
            </label>

            <label className="auth-modal__field">
              <span>Verification code</span>
              <input
                ref={tokenRef}
                className="field__input field__input--ui"
                type="text"
                autoComplete="one-time-code"
                inputMode="text"
                spellCheck={false}
                value={token}
                onChange={(event) => {
                  setToken(event.target.value);
                  if (errorField === "token") clearError();
                }}
                placeholder="Paste the 64-character code from your email"
                disabled={busy}
                data-auth-autofocus
                {...errorProps("token")}
              />
            </label>

            <button
              type="submit"
              className="button button--primary auth-modal__submit"
              disabled={busy || authServiceConfigured === false}
              aria-busy={busy}
            >
              {busy ? "Verifying…" : "Verify email"}
            </button>

            <div className="auth-modal__footer-links">
              <button type="button" className="auth-modal__link" onClick={() => void handleResendVerification()} disabled={busy}>
                {busy ? "Requesting…" : "Request another verification message"}
              </button>
              <button type="button" className="auth-modal__link" onClick={() => switchMode("signin")} disabled={busy}>
                Back to sign in
              </button>
            </div>
          </form>
        ) : null}

        {mode === "recover" ? (
          <form className="auth-modal__form" onSubmit={(event) => void handleRecover(event)} noValidate>
            <label className="auth-modal__field">
              <span>Email address</span>
              <input
                ref={emailRef}
                className="field__input field__input--ui"
                type="email"
                autoComplete="email"
                autoCapitalize="none"
                inputMode="email"
                spellCheck={false}
                value={email}
                onChange={(event) => {
                  setEmail(event.target.value);
                  if (errorField === "email") clearError();
                }}
                placeholder="you@example.com"
                disabled={busy}
                required
                data-auth-autofocus
                {...errorProps("email")}
              />
            </label>

            <button
              type="submit"
              className="button button--primary auth-modal__submit"
              disabled={busy || authServiceConfigured === false}
              aria-busy={busy}
            >
              {busy ? "Requesting…" : "Request password reset"}
            </button>

            <div className="auth-modal__footer-links">
              <button type="button" className="auth-modal__link" onClick={() => switchMode("reset")} disabled={busy}>
                Already have a reset token?
              </button>
              <button type="button" className="auth-modal__link" onClick={() => switchMode("signin")} disabled={busy}>
                Back to sign in
              </button>
            </div>
          </form>
        ) : null}

        {mode === "reset" ? (
          <form className="auth-modal__form" onSubmit={(event) => void handleReset(event)} noValidate>
            <label className="auth-modal__field">
              <span>Reset token</span>
              <input
                ref={tokenRef}
                className="field__input field__input--ui"
                type="text"
                autoComplete="one-time-code"
                spellCheck={false}
                value={token}
                onChange={(event) => {
                  setToken(event.target.value);
                  if (errorField === "token") clearError();
                }}
                placeholder="Paste the 64-character reset token"
                disabled={busy}
                required
                data-auth-autofocus
                {...errorProps("token")}
              />
            </label>

            <label className="auth-modal__field">
              <span>New password</span>
              <input
                ref={passwordRef}
                className="field__input field__input--ui"
                type="password"
                autoComplete="new-password"
                value={password}
                onChange={(event) => {
                  setPassword(event.target.value);
                  if (errorField === "password") clearError();
                }}
                placeholder={`At least ${PASSWORD_MIN_LENGTH} characters`}
                disabled={busy}
                required
                {...errorProps("password")}
              />
            </label>

            <button
              type="submit"
              className="button button--primary auth-modal__submit"
              disabled={busy || authServiceConfigured === false}
              aria-busy={busy}
            >
              {busy ? "Updating…" : "Update password"}
            </button>

            <div className="auth-modal__footer-links">
              <button type="button" className="auth-modal__link" onClick={() => switchMode("signin")} disabled={busy}>
                Back to sign in
              </button>
            </div>
          </form>
        ) : null}
      </div>
    </div>
  );
}
