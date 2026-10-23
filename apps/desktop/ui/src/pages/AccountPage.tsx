import { useState, type FormEvent } from "react";
import type { AuthModalMode } from "../components/AuthModal";
import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";
import { useAuth } from "../features/auth/useAuth";
import { navigateTo } from "../lib/router";
import { PageHeader } from "./PageHeader";

interface AccountPageProps {
  onOpenAuthModal?: (mode: AuthModalMode) => void;
}

export function AccountPage({ onOpenAuthModal }: AccountPageProps = {}) {
  const { user, isAuthenticated, loading, logout, changePassword, resendVerification } = useAuth();

  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [pwdStatus, setPwdStatus] = useState<{ type: "success" | "error"; message: string } | null>(
    null,
  );
  const [changingPwd, setChangingPwd] = useState(false);

  const [resendStatus, setResendStatus] = useState<string | null>(null);
  const [resending, setResending] = useState(false);

  async function handleLogout() {
    await logout();
    navigateTo("chat");
  }

  async function handleChangePassword(e: FormEvent) {
    e.preventDefault();
    setPwdStatus(null);

    if (newPassword.length < 8) {
      setPwdStatus({ type: "error", message: "New password must be at least 8 characters long." });
      return;
    }

    if (newPassword !== confirmPassword) {
      setPwdStatus({ type: "error", message: "Passwords do not match." });
      return;
    }

    setChangingPwd(true);
    try {
      const resp = await changePassword({
        currentPassword,
        newPassword,
      });
      setPwdStatus({ type: "success", message: resp.message || "Password changed successfully." });
      setCurrentPassword("");
      setNewPassword("");
      setConfirmPassword("");
    } catch (err: unknown) {
      const msg =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : "Failed to change password.";
      setPwdStatus({ type: "error", message: msg });
    } finally {
      setChangingPwd(false);
    }
  }

  async function handleResendVerification() {
    if (!user?.email) return;
    setResending(true);
    setResendStatus(null);
    try {
      const resp = await resendVerification(user.email);
      setResendStatus(resp.message || "Verification email sent.");
    } catch (err: unknown) {
      const msg =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : "Could not resend verification email.";
      setResendStatus(msg);
    } finally {
      setResending(false);
    }
  }

  return (
    <div className="page page--account">
      <PageHeader
        eyebrow="Account"
        title="Developer Account"
        copy="Manage your Nexora account identity and security settings. Local inference and conversation history stay on your device."
      />

      <div className="page-stack">
        <section className="settings-group" aria-label="Account profile">
          <h2 className="settings-group__title">Profile</h2>

          {loading ? (
            <p className="settings-row__hint">Checking session status…</p>
          ) : !isAuthenticated || !user ? (
            <div className="settings-row settings-row--static">
              <span>
                <span className="settings-row__label">Local Guest Mode (Unauthenticated)</span>
                <span className="settings-row__hint">
                  Local AI chat, conversation search, and workspace tools work on this machine without signing in. Sign in is optional for account management and server plan sync.
                </span>
              </span>
              <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                <RouterLink
                  to="signin"
                  className="button button--primary"
                  onClick={
                    onOpenAuthModal
                      ? (e) => {
                          e.preventDefault();
                          onOpenAuthModal("signin");
                        }
                      : undefined
                  }
                >
                  Sign in
                </RouterLink>
                <RouterLink
                  to="signup"
                  className="button button--ghost"
                  onClick={
                    onOpenAuthModal
                      ? (e) => {
                          e.preventDefault();
                          onOpenAuthModal("signup");
                        }
                      : undefined
                  }
                >
                  Create account
                </RouterLink>
              </div>
            </div>
          ) : (
            <>
              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">Display name</span>
                  <span className="settings-row__hint">Your Nexora account display name.</span>
                </span>
                <span className="settings-row__value">{user.name}</span>
              </div>

              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">Email address</span>
                  <span className="settings-row__hint">Used for account authentication and recovery.</span>
                </span>
                <span className="settings-row__value">{user.email}</span>
              </div>

              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">Account ID</span>
                  <span className="settings-row__hint">Unique identifier for your Nexora identity.</span>
                </span>
                <span className="settings-row__value mono">{user.id}</span>
              </div>

              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">Email verification</span>
                  <span className="settings-row__hint">
                    {user.emailVerified
                      ? "Confirmed for account recovery and notifications."
                      : "Please confirm your email address."}
                  </span>
                </span>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  {user.emailVerified ? (
                    <span className="badge badge--local">
                      <Icon name="check" size={12} /> Verified
                    </span>
                  ) : (
                    <>
                      <span className="badge">Unverified</span>
                      <button
                        type="button"
                        className="button button--ghost button--sm"
                        disabled={resending}
                        onClick={handleResendVerification}
                      >
                        {resending ? "Sending…" : "Resend email"}
                      </button>
                    </>
                  )}
                </div>
              </div>

              {resendStatus && (
                <div className="status status--info" role="status" style={{ marginTop: 8 }}>
                  {resendStatus}
                </div>
              )}

              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">Active Session</span>
                  <span className="settings-row__hint">
                    Signing out clears your account token from OS credential storage while keeping local conversations intact.
                  </span>
                </span>
                <button type="button" className="button button--ghost" onClick={handleLogout}>
                  Sign out
                </button>
              </div>
            </>
          )}
        </section>

        {isAuthenticated && user && (
          <section className="settings-group" aria-label="Password security">
            <h2 className="settings-group__title">Change password</h2>

            <form
              onSubmit={handleChangePassword}
              style={{ display: "flex", flexDirection: "column", gap: 12, maxWidth: 420 }}
            >
              {pwdStatus && (
                <div
                  className={`status status--${pwdStatus.type === "error" ? "error" : "success"}`}
                  role="alert"
                >
                  {pwdStatus.message}
                </div>
              )}

              <label className="field">
                <span className="field__label">Current password</span>
                <input
                  className="field__input"
                  type="password"
                  required
                  value={currentPassword}
                  onChange={(e) => setCurrentPassword(e.target.value)}
                />
              </label>

              <label className="field">
                <span className="field__label">New password (at least 8 characters)</span>
                <input
                  className="field__input"
                  type="password"
                  required
                  minLength={8}
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                />
              </label>

              <label className="field">
                <span className="field__label">Confirm new password</span>
                <input
                  className="field__input"
                  type="password"
                  required
                  minLength={8}
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                />
              </label>

              <div>
                <button type="submit" className="button button--primary" disabled={changingPwd}>
                  {changingPwd ? "Updating…" : "Update password"}
                </button>
              </div>
            </form>
          </section>
        )}

        <section className="settings-group" aria-label="Privacy guarantee">
          <h2 className="settings-group__title">Local Privacy Boundary</h2>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Zero cloud code transmission</span>
              <span className="settings-row__hint">
                Your source code, prompts, model weights, and local conversation transcripts remain on your machine.
              </span>
            </span>
          </div>
        </section>
      </div>
    </div>
  );
}
