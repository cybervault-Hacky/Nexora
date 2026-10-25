/** Authentication feedback that never echoes credentials, tokens, or account addresses. */

export type AuthFlow = "signin" | "signup" | "verify" | "recover" | "reset" | "google" | "general";

interface ErrorShape {
  code: string;
  message: string;
  retryAfterSeconds?: number;
}

function readErrorShape(error: unknown): ErrorShape {
  if (typeof error !== "object" || error === null) {
    return { code: "", message: "" };
  }

  const candidate = error as Record<string, unknown>;
  return {
    code: typeof candidate.code === "string" ? candidate.code.toLowerCase() : "",
    message: typeof candidate.message === "string" ? candidate.message : "",
    retryAfterSeconds:
      typeof candidate.retryAfterSeconds === "number" && Number.isFinite(candidate.retryAfterSeconds)
        ? candidate.retryAfterSeconds
        : undefined,
  };
}

function containsSensitiveValue(message: string, sensitiveValues: readonly string[]): boolean {
  const normalized = message.toLocaleLowerCase();
  if (
    sensitiveValues.some((value) => {
      const secret = value.trim().toLocaleLowerCase();
      return secret.length >= 4 && normalized.includes(secret);
    })
  ) {
    return true;
  }

  return (
    /\b[\w.+-]+@[\w.-]+\.[a-z]{2,}\b/i.test(message) ||
    /\bbearer\s+[a-z0-9._~+/-]+=*/i.test(message) ||
    /\beyJ[a-zA-Z0-9_-]{8,}\.[a-zA-Z0-9_-]{8,}\.[a-zA-Z0-9_-]{8,}\b/.test(message) ||
    /\b[a-f0-9]{64}\b/i.test(message) ||
    /\b(?:password|token|secret|cookie)\s*[:=]\s*\S+/i.test(message)
  );
}

/**
 * A service may return a useful generic status sentence. Keep it as text, bound
 * its size, and discard it if it contains private data or infrastructure detail.
 */
export function safeServiceMessage(
  value: unknown,
  sensitiveValues: readonly string[] = [],
): string | undefined {
  if (typeof value !== "string") return undefined;
  const message = value.replace(/[\u0000-\u001f\u007f]/g, " ").trim();
  if (
    message.length === 0 ||
    message.length > 320 ||
    containsSensitiveValue(message, sensitiveValues) ||
    /\b(?:sqlx|postgres(?:ql)?|database connection|stack trace|panic|exception|backtrace|internal server error|econn(?:refused|reset)|connection refused)\b/i.test(
      message,
    ) ||
    /(?:https?|wss?|file):\/\/\S+|\b(?:localhost|127(?:\.\d{1,3}){3}|::1)\b|\b(?:[a-z0-9-]+\.)+[a-z]{2,63}(?::\d{2,5})?(?:\/\S*)?/i.test(
      message,
    ) ||
    /\b(?:[a-z0-9-]+\.)*[a-z0-9-]+:\d{2,5}(?:\/\S*)?/i.test(message)
  ) {
    return undefined;
  }
  return message;
}

function fallbackFor(flow: AuthFlow): string {
  switch (flow) {
    case "signin":
      return "Sign-in could not be completed. Check your details or try again later.";
    case "signup":
      return "Account creation could not be confirmed. Try again, or sign in if you may already have an account.";
    case "verify":
      return "Email verification could not be completed. Check the code or try again later.";
    case "recover":
      return "Password recovery could not be completed. No email delivery was confirmed.";
    case "reset":
      return "Password reset could not be completed. The password change was not confirmed.";
    case "google":
      return "Google sign-in could not be completed. Try again or use email sign-in.";
    default:
      return "The account request could not be completed. Try again later.";
  }
}

/** Convert contract error codes to actionable, privacy-safe copy for the active flow. */
export function safeAuthErrorMessage(
  error: unknown,
  flow: AuthFlow,
  sensitiveValues: readonly string[] = [],
): string {
  const { code, message, retryAfterSeconds } = readErrorShape(error);

  if (code === "invalid_credentials" || code === "invalid_login") {
    return "Invalid email or password.";
  }
  if (code === "email_unverified" || code === "email_not_verified") {
    return "Verify your email address before signing in. You can enter a verification code here.";
  }
  if (code === "email_exists") {
    return "An account with this email may already exist. Try signing in or use password recovery.";
  }
  if (code === "rate_limited") {
    const retryMinutes = Math.max(1, Math.ceil(Math.min(retryAfterSeconds ?? 60, 86_400) / 60));
    return `Too many attempts. Wait about ${retryMinutes} minute${retryMinutes === 1 ? "" : "s"} before trying again.`;
  }
  if (code === "network_error" || code === "timeout") {
    return "Can't reach the Nexora account service. Check your connection and try again.";
  }
  if (code === "backend_unconfigured" || code === "invalid_backend_url") {
    return "Account authentication isn't configured in this build. Local chat and workspace tools remain available.";
  }
  if (code === "desktop_required") {
    return "Account sign-in is available in the Nexora desktop app, not in browser preview.";
  }
  if (code === "dependency_unavailable") {
    if (flow === "google") {
      return "Google sign-in isn't configured for this account service. Use email sign-in or contact your administrator.";
    }
    if (flow === "recover") {
      return "Password recovery isn't available right now. No reset email was confirmed.";
    }
    if (flow === "verify") {
      return "Email verification isn't available right now. No verification email was confirmed.";
    }
    if (flow === "reset") {
      return "Password reset isn't available right now. Your password change was not confirmed.";
    }
    return "The account service is temporarily unavailable. Try again later.";
  }
  if (code === "invalid_or_expired_token" || code === "invalid_token" || code === "expired_token") {
    return flow === "verify"
      ? "This verification code is invalid or expired. Request a new code if needed."
      : "This reset token is invalid or expired. Request a new reset link if needed.";
  }
  if (code === "email_conflict") {
    return "This Google identity can't be linked automatically. Sign in with the existing account method.";
  }
  if (code === "cancelled") {
    return "";
  }
  if (code === "internal" || code.startsWith("http_5") || code === "storage_error") {
    return fallbackFor(flow);
  }

  const safeMessage = safeServiceMessage(message, sensitiveValues);
  if (safeMessage) return safeMessage;
  return fallbackFor(flow);
}

export function authErrorCode(error: unknown): string {
  return readErrorShape(error).code;
}
