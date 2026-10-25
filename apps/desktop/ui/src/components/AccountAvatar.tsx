import type { AuthUser } from "../lib/auth";

/** The account API has no avatar-image field; initials are the supported fallback. */
export function AccountAvatar({ user, large = false }: { user: AuthUser; large?: boolean }) {
  const initials = (user.name.trim() || (user.email.split("@")[0] ?? ""))
    .split(/\s+/).slice(0, 2).map((part) => Array.from(part)[0]?.toUpperCase() ?? "").join("");
  return <span className={`account-avatar${large ? " account-avatar--large" : ""}`} aria-hidden="true">{initials}</span>;
}
