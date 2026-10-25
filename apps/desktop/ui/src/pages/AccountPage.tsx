import { useEffect, useState, type FormEvent } from "react";
import type { AuthModalMode } from "../components/AuthModal";
import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";
import { useAuth, type AuthController } from "../features/auth/useAuth";
import {
  accountGetProfile,
  accountUpdateProfile,
  isAccountProfile,
  type AccountProfile,
} from "../lib/auth";
import { authErrorCode, safeAuthErrorMessage } from "../lib/authMessages";
import { navigateTo } from "../lib/router";
import { PageHeader } from "./PageHeader";

const MAX_PROFILE_NAME_CHARACTERS = 80;

type StatusMessage = { type: "success" | "error"; message: string };

function isInvalidSessionError(error: unknown): boolean {
  return [
    "session_expired",
    "invalid_or_expired_token",
    "invalid_token",
    "expired_token",
    "unauthenticated",
  ].includes(authErrorCode(error));
}

function profileErrorMessage(error: unknown): string {
  const code = authErrorCode(error);
  if (isInvalidSessionError(error)) {
    return "Your Nexora session is no longer valid. Sign in again to access your account.";
  }
  if (["network_error", "timeout", "dependency_unavailable"].includes(code)) {
    return "Your account profile is temporarily unavailable. Check your connection and try again.";
  }
  return safeAuthErrorMessage(error, "general");
}

function validateProfileName(name: string): string | null {
  const trimmed = name.trim();
  const characterCount = Array.from(trimmed).length;
  if (characterCount === 0) return "Display name is required.";
  if (characterCount > MAX_PROFILE_NAME_CHARACTERS) {
    return `Display name must be ${MAX_PROFILE_NAME_CHARACTERS} characters or fewer.`;
  }
  if (
    Array.from(trimmed).some((character) => {
      const codePoint = character.codePointAt(0) ?? 0;
      return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f);
    })
  ) {
    return "Display name must not contain control characters.";
  }
  return null;
}

function accountProviderLabels(profile: AccountProfile): string {
  const labels = Array.from(
    new Set(
      profile.providers.map((provider) => {
        switch (provider.toLowerCase()) {
          case "password":
            return "Email and password";
          case "google":
            return "Google";
          default:
            return "Other connected sign-in method";
        }
      }),
    ),
  );
  return labels.length > 0 ? labels.join(", ") : "Not available";
}

function formatAccountCreatedAt(value: string): string {
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return "Not available";
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(timestamp);
}

interface AccountPageProps {
  onOpenAuthModal?: (mode: AuthModalMode) => void;
  /** App-owned controller keeps account view state in sync with the modal session. */
  auth?: AuthController;
}

interface AccountPageViewProps {
  onOpenAuthModal?: (mode: AuthModalMode) => void;
  auth: AuthController;
}

export function AccountPage(props: AccountPageProps = {}) {
  if (props.auth) {
    return <AccountPageView auth={props.auth} onOpenAuthModal={props.onOpenAuthModal} />;
  }
  return <AccountPageWithLocalAuth onOpenAuthModal={props.onOpenAuthModal} />;
}

function AccountPageWithLocalAuth({ onOpenAuthModal }: AccountPageProps) {
  const auth = useAuth();
  return <AccountPageView auth={auth} onOpenAuthModal={onOpenAuthModal} />;
}

function AccountPageView({ onOpenAuthModal, auth }: AccountPageViewProps) {
  const { user, isAuthenticated, loading, logout, changePassword, resendVerification } = auth;
  const [profile, setProfile] = useState<AccountProfile | null>(null);
  const [profileLoading, setProfileLoading] = useState(false);
  const [profileError, setProfileError] = useState<string | null>(null);
  const [profileLoadAttempt, setProfileLoadAttempt] = useState(0);
  const [editingName, setEditingName] = useState(false);
  const [nameDraft, setNameDraft] = useState("");
  const [savingProfile, setSavingProfile] = useState(false);
  const [profileStatus, setProfileStatus] = useState<StatusMessage | null>(null);

  useEffect(() => {
    let current = true;
    const sessionUserId = user?.id;

    setProfile(null);
    setProfileError(null);
    setProfileStatus(null);
    setEditingName(false);
    setNameDraft("");

    if (loading || !isAuthenticated || !sessionUserId) {
      setProfileLoading(false);
      return () => {
        current = false;
      };
    }

    setProfileLoading(true);
    void (async () => {
      try {
        const response = await accountGetProfile();
        if (!isAccountProfile(response) || response.id !== sessionUserId) {
          throw {
            code: "invalid_response",
            message: "The account service returned an invalid profile response.",
          };
        }
        if (current) setProfile(response);
      } catch (error: unknown) {
        if (isInvalidSessionError(error)) {
          await auth.refreshSession().catch(() => undefined);
        }
        if (current) setProfileError(profileErrorMessage(error));
      } finally {
        if (current) setProfileLoading(false);
      }
    })();

    return () => {
      current = false;
    };
  }, [auth.refreshSession, isAuthenticated, loading, profileLoadAttempt, user?.id]);

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

  function beginNameEdit() {
    if (!profile) return;
    setNameDraft(profile.name);
    setProfileStatus(null);
    setEditingName(true);
  }

  function cancelNameEdit() {
    setNameDraft(profile?.name ?? "");
    setProfileStatus(null);
    setEditingName(false);
  }

  async function handleUpdateProfile(e: FormEvent) {
    e.preventDefault();
    if (!profile || savingProfile) return;

    const nextName = nameDraft.trim();
    const validationMessage = validateProfileName(nameDraft);
    if (validationMessage) {
      setProfileStatus({ type: "error", message: validationMessage });
      return;
    }

    setProfileStatus(null);
    setSavingProfile(true);
    try {
      const updated = await accountUpdateProfile({ name: nextName });
      if (!isAccountProfile(updated) || updated.id !== profile.id) {
        throw {
          code: "invalid_response",
          message: "The account service returned an invalid profile response.",
        };
      }
      setProfile(updated);
      setNameDraft(updated.name);
      setEditingName(false);
      setProfileStatus({ type: "success", message: "Display name updated." });
      void auth.refreshSession().catch(() => undefined);
    } catch (error: unknown) {
      if (isInvalidSessionError(error)) {
        void auth.refreshSession().catch(() => undefined);
      }
      setProfileStatus({ type: "error", message: profileErrorMessage(error) });
    } finally {
      setSavingProfile(false);
    }
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
    if (!profile?.email) return;
    setResending(true);
    setResendStatus(null);
    try {
      const resp = await resendVerification(profile.email);
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

  const visibleProfile =
    isAuthenticated && user && profile?.id === user.id ? profile : null;

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
              {profileLoading && (
                <p className="settings-row__hint" role="status">
                  Loading your account profile…
                </p>
              )}

              {profileError && (
                <div className="status status--error" role="alert">
                  <p>{profileError}</p>
                  <button
                    type="button"
                    className="button button--ghost button--sm"
                    disabled={profileLoading}
                    onClick={() => setProfileLoadAttempt((attempt) => attempt + 1)}
                  >
                    Retry profile load
                  </button>
                </div>
              )}

              {visibleProfile && (
                <>
                  <div className="settings-row settings-row--static">
                    <span>
                      <span className="settings-row__label">Display name</span>
                      <span className="settings-row__hint">Your Nexora account display name.</span>
                    </span>
                    {editingName ? (
                      <form
                        aria-label="Update display name"
                        onSubmit={handleUpdateProfile}
                        style={{ display: "flex", flexDirection: "column", gap: 8, width: "100%", maxWidth: 420 }}
                      >
                        <label className="field">
                          <span className="field__label">Display name</span>
                          <input
                            className="field__input"
                            type="text"
                            autoComplete="name"
                            maxLength={MAX_PROFILE_NAME_CHARACTERS * 2}
                            required
                            disabled={savingProfile}
                            value={nameDraft}
                            onChange={(event) => setNameDraft(event.target.value)}
                          />
                        </label>
                        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                          <button type="submit" className="button button--primary" disabled={savingProfile}>
                            {savingProfile ? "Saving…" : "Save name"}
                          </button>
                          <button
                            type="button"
                            className="button button--ghost"
                            disabled={savingProfile}
                            onClick={cancelNameEdit}
                          >
                            Cancel
                          </button>
                        </div>
                      </form>
                    ) : (
                      <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                        <span className="settings-row__value">{visibleProfile.name}</span>
                        <button
                          type="button"
                          className="button button--ghost button--sm"
                          onClick={beginNameEdit}
                        >
                          Edit name
                        </button>
                      </div>
                    )}
                  </div>

                  {profileStatus && (
                    <div
                      className={`status status--${profileStatus.type === "error" ? "error" : "info"}`}
                      role={profileStatus.type === "error" ? "alert" : "status"}
                      style={{ marginTop: 8 }}
                    >
                      {profileStatus.message}
                    </div>
                  )}

                  <div className="settings-row settings-row--static">
                    <span>
                      <span className="settings-row__label">Email address</span>
                      <span className="settings-row__hint">Used for account authentication and recovery.</span>
                    </span>
                    <span className="settings-row__value">{visibleProfile.email}</span>
                  </div>

                  <div className="settings-row settings-row--static">
                    <span>
                      <span className="settings-row__label">Account ID</span>
                      <span className="settings-row__hint">Unique identifier for your Nexora identity.</span>
                    </span>
                    <span className="settings-row__value mono">{visibleProfile.id}</span>
                  </div>

                  <div className="settings-row settings-row--static">
                    <span>
                      <span className="settings-row__label">Sign-in methods</span>
                      <span className="settings-row__hint">Authentication methods linked to this account.</span>
                    </span>
                    <span className="settings-row__value">{accountProviderLabels(visibleProfile)}</span>
                  </div>

                  <div className="settings-row settings-row--static">
                    <span>
                      <span className="settings-row__label">Account created</span>
                      <span className="settings-row__hint">Date the Nexora account was created.</span>
                    </span>
                    <span className="settings-row__value">{formatAccountCreatedAt(visibleProfile.createdAt)}</span>
                  </div>

                  <div className="settings-row settings-row--static">
                    <span>
                      <span className="settings-row__label">Email verification</span>
                      <span className="settings-row__hint">
                        {visibleProfile.emailVerified
                          ? "Confirmed for account recovery and notifications."
                          : "Please confirm your email address."}
                      </span>
                    </span>
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                      {visibleProfile.emailVerified ? (
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
                </>
              )}

              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">Active Session</span>
                  <span className="settings-row__hint">
                    Signing out clears your account token from OS credential storage while keeping local conversations intact.
                  </span>
                </span>
                <button
                  type="button"
                  className="button button--ghost"
                  disabled={savingProfile}
                  onClick={handleLogout}
                >
                  Sign out
                </button>
              </div>
            </>
          )}
        </section>

        {!loading && visibleProfile?.hasPassword && (
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
