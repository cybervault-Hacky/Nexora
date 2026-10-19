import { useState, type FormEvent } from "react";
import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";
import { useAuth } from "../features/auth/useAuth";
import { PASSWORD_MIN_LENGTH, validatePasswordConfirmation } from "../lib/formValidation";
import { PageHeader } from "./PageHeader";

interface StatusNotice {
  tone: "info" | "success" | "error";
  message: string;
}

/**
 * Phase 25 — Developer Account & Profile Management
 *
 * Provides developers and founders with verified identity details, password
 * updates, verification management, and secure OS session termination.
 * Unauthenticated guest users are presented with clear guest status and
 * seamless links to sign in, preserving complete offline AI functionality.
 */
export function AccountPage() {
  const { user, isAuthenticated, loading, logout, changePassword, resendVerification } = useAuth();

  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [showPasswords, setShowPasswords] = useState(false);
  const [submittingPassword, setSubmittingPassword] = useState(false);
  const [passwordNotice, setPasswordNotice] = useState<StatusNotice | null>(null);

  const [resending, setResending] = useState(false);
  const [resendNotice, setResendNotice] = useState<StatusNotice | null>(null);

  const handlePasswordSubmit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (submittingPassword) return;

    setPasswordNotice(null);

    if (!currentPassword) {
      setPasswordNotice({ tone: "error", message: "Enter your current password." });
      return;
    }

    if (newPassword.length < PASSWORD_MIN_LENGTH) {
      setPasswordNotice({
        tone: "error",
        message: `New password must be at least ${PASSWORD_MIN_LENGTH} characters.`,
      });
      return;
    }

    const matchErr = validatePasswordConfirmation(newPassword, confirmPassword);
    if (matchErr) {
      setPasswordNotice({ tone: "error", message: matchErr });
      return;
    }

    setSubmittingPassword(true);
    try {
      const res = await changePassword({ currentPassword, newPassword });
      setCurrentPassword("");
      setNewPassword("");
      setConfirmPassword("");
      setPasswordNotice({ tone: "success", message: res.message || "Password updated successfully." });
    } catch (err: unknown) {
      const msg = typeof err === "object" && err !== null && "message" in err
        ? String((err as { message: unknown }).message)
        : "Failed to update password. Check your current password and try again.";
      setPasswordNotice({ tone: "error", message: msg });
    } finally {
      setSubmittingPassword(false);
    }
  };

  const handleResendVerification = async () => {
    if (!user || resending) return;
    setResending(true);
    setResendNotice(null);
    try {
      const res = await resendVerification(user.email);
      setResendNotice({ tone: "info", message: res.message || "Verification email sent if account is unverified." });
    } catch (err: unknown) {
      const msg = typeof err === "object" && err !== null && "message" in err
        ? String((err as { message: unknown }).message)
        : "Could not send verification email. Try again shortly.";
      setResendNotice({ tone: "error", message: msg });
    } finally {
      setResending(false);
    }
  };

  const handleSignOut = async () => {
    await logout();
  };

  return (
    <div className="page page--account">
      <PageHeader
        eyebrow="Identity & Security"
        title="Developer Account"
        copy="Manage your developer profile, credential security, and session status. Local AI, code intelligence, and workspace tools remain completely functional without requiring authentication."
      />

      <div className="page-stack">
        {/* Profile / Guest Status Section */}
        <section className="settings-group" aria-label="Account details">
          <h2 className="settings-group__title">Profile Status</h2>

          {loading ? (
            <div className="settings-row settings-row--static">
              <span>Checking credential vault…</span>
            </div>
          ) : isAuthenticated && user ? (
            <>
              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">Developer Name</span>
                  <span className="settings-row__hint">{user.name || "Unnamed developer"}</span>
                </span>
                <span className="badge badge--local">Active Session</span>
              </div>

              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">Email address</span>
                  <span className="settings-row__hint">{user.email}</span>
                </span>
                {user.emailVerified ? (
                  <span className="badge badge--success" style={{ color: "var(--accent-success, #34d399)" }}>
                    <Icon name="check" size={12} /> Verified
                  </span>
                ) : (
                  <span className="badge badge--warning" style={{ color: "var(--accent-warning, #fbbf24)" }}>
                    <Icon name="alert" size={12} /> Unverified
                  </span>
                )}
              </div>

              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">Account Identifier</span>
                  <span className="settings-row__hint" style={{ fontFamily: "monospace" }}>
                    {user.id}
                  </span>
                </span>
              </div>

              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">Developer Edition & Licencing</span>
                  <span className="settings-row__hint">
                    Manage your edition, commercial rights, and offline activation keys.
                  </span>
                </span>
                <RouterLink to="licensing" className="button button--ghost button--sm">
                  <Icon name="shield" size={12} /> Manage Licence
                </RouterLink>
              </div>

              {!user.emailVerified && (
                <div className="settings-row settings-row--static">
                  <span>
                    <span className="settings-row__label">Email verification</span>
                    <span className="settings-row__hint">
                      Your email address has not been verified yet. Check your inbox for the link.
                    </span>
                    {resendNotice && (
                      <span className={`status status--${resendNotice.tone}`} style={{ display: "block", marginTop: 4 }}>
                        {resendNotice.message}
                      </span>
                    )}
                  </span>
                  <button
                    type="button"
                    className="button button--ghost button--sm"
                    disabled={resending}
                    onClick={handleResendVerification}
                  >
                    <Icon name="refresh" size={13} /> {resending ? "Sending…" : "Resend email"}
                  </button>
                </div>
              )}
            </>
          ) : (
            <div className="settings-row settings-row--static" style={{ flexDirection: "column", alignItems: "flex-start", gap: 12 }}>
              <span>
                <span className="settings-row__label">Local Guest Mode (Unauthenticated)</span>
                <span className="settings-row__hint">
                  You are not currently signed in. Nexora works offline-first: local models, conversations, projects,
                  and code analysis are completely unrestricted.
                </span>
              </span>
              <div style={{ display: "flex", gap: 8, marginTop: 4 }}>
                <RouterLink to="signin" className="button button--primary button--sm">
                  Sign in
                </RouterLink>
                <RouterLink to="signup" className="button button--ghost button--sm">
                  Create account
                </RouterLink>
              </div>
            </div>
          )}
        </section>

        {/* Password Security Section (Authenticated users) */}
        {isAuthenticated && (
          <section className="settings-group" aria-label="Password security">
            <h2 className="settings-group__title">Change Password</h2>
            <form onSubmit={handlePasswordSubmit} style={{ display: "flex", flexDirection: "column", gap: 12 }}>
              {passwordNotice && (
                <div className={`status status--${passwordNotice.tone}`} role="status">
                  {passwordNotice.message}
                </div>
              )}

              <div className="settings-row settings-row--static" style={{ flexDirection: "column", alignItems: "flex-start", gap: 6 }}>
                <label htmlFor="account-current-pw" className="settings-row__label">
                  Current password
                </label>
                <input
                  id="account-current-pw"
                  type={showPasswords ? "text" : "password"}
                  value={currentPassword}
                  onChange={(e) => setCurrentPassword(e.target.value)}
                  className="input"
                  style={{ width: "100%", maxWidth: 360 }}
                  placeholder="Enter current password"
                  autoComplete="current-password"
                  disabled={submittingPassword}
                />
              </div>

              <div className="settings-row settings-row--static" style={{ flexDirection: "column", alignItems: "flex-start", gap: 6 }}>
                <label htmlFor="account-new-pw" className="settings-row__label">
                  New password (at least 8 characters)
                </label>
                <input
                  id="account-new-pw"
                  type={showPasswords ? "text" : "password"}
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                  className="input"
                  style={{ width: "100%", maxWidth: 360 }}
                  placeholder="Enter new password"
                  autoComplete="new-password"
                  disabled={submittingPassword}
                />
              </div>

              <div className="settings-row settings-row--static" style={{ flexDirection: "column", alignItems: "flex-start", gap: 6 }}>
                <label htmlFor="account-confirm-pw" className="settings-row__label">
                  Confirm new password
                </label>
                <input
                  id="account-confirm-pw"
                  type={showPasswords ? "text" : "password"}
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  className="input"
                  style={{ width: "100%", maxWidth: 360 }}
                  placeholder="Confirm new password"
                  autoComplete="new-password"
                  disabled={submittingPassword}
                />
              </div>

              <div style={{ display: "flex", alignItems: "center", gap: 12, marginTop: 4 }}>
                <button
                  type="button"
                  className="button button--ghost button--sm"
                  onClick={() => setShowPasswords(!showPasswords)}
                >
                  <Icon name={showPasswords ? "eye-off" : "eye"} size={13} />
                  {showPasswords ? "Hide passwords" : "Show passwords"}
                </button>
                <button
                  type="submit"
                  className="button button--primary button--sm"
                  disabled={submittingPassword || !currentPassword || !newPassword}
                >
                  {submittingPassword ? "Updating…" : "Update password"}
                </button>
              </div>
            </form>
          </section>
        )}

        {/* Session Management */}
        {isAuthenticated && (
          <section className="settings-group" aria-label="Session management">
            <h2 className="settings-group__title">Session & Sign Out</h2>
            <div className="settings-row settings-row--static">
              <span>
                <span className="settings-row__label">Active OS Session</span>
                <span className="settings-row__hint">
                  Session tokens are encrypted in the OS Credential Manager. Signing out revokes
                  the session and purges local credentials.
                </span>
              </span>
              <button
                type="button"
                className="button button--ghost button--sm"
                onClick={handleSignOut}
              >
                Sign out
              </button>
            </div>
          </section>
        )}
      </div>
    </div>
  );
}
