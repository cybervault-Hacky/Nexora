/**
 * Shared, purely local form validation for the account pages.
 *
 * Rules stay deliberately simple (NIST SP 800-63B style): a length floor for
 * new passwords and no composition rules, since forced complexity has no clear
 * security justification. Everything here runs in the page — nothing is sent.
 */

/** Minimum length for NEW passwords (sign-in never enforces length). */
export const PASSWORD_MIN_LENGTH = 8;

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

/** Local, purely syntactic email check — no deliverability claims. */
export function validateEmail(value: string): string | undefined {
  const trimmed = value.trim();
  if (trimmed.length === 0) return "Enter your email address.";
  if (!EMAIL_PATTERN.test(trimmed)) return "Enter a valid email address.";
  return undefined;
}

export function validateFullName(value: string): string | undefined {
  if (value.trim().length === 0) return "Enter your full name.";
  return undefined;
}

/** New-password rule: non-empty and at least {@link PASSWORD_MIN_LENGTH} characters. */
export function validateNewPassword(value: string): string | undefined {
  if (value.length === 0) return "Enter your password.";
  if (value.length < PASSWORD_MIN_LENGTH) return `Use at least ${PASSWORD_MIN_LENGTH} characters.`;
  return undefined;
}

export function validatePasswordConfirmation(
  password: string,
  confirmation: string,
): string | undefined {
  if (confirmation.length === 0) return "Re-enter your password.";
  if (confirmation !== password) return "Passwords do not match.";
  return undefined;
}
