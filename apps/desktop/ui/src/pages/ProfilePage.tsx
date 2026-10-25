import { useEffect, useState, type FormEvent } from "react";
import { AccountAvatar } from "../components/AccountAvatar";
import { RouterLink } from "../components/RouterLink";
import { accountGetProfile, accountUpdateProfile, isAccountProfile, type AccountProfile, type AuthUser } from "../lib/auth";
import { safeAuthErrorMessage } from "../lib/authMessages";

/** Private, authenticated profile. The account contract supports a name only, not handles or uploads. */
export function ProfilePage({ user, onRequestSignIn, onProfileSaved }: {
  user: AuthUser | null; onRequestSignIn: () => void; onProfileSaved: () => void;
}) {
  const [profile, setProfile] = useState<AccountProfile | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  useEffect(() => {
    let active = true;
    setProfile(null); setError(null); setEditing(false); setNotice(null);
    if (!user) { setLoading(false); return; }
    setLoading(true);
    void accountGetProfile().then((result) => {
      if (!active) return;
      if (!isAccountProfile(result) || result.id !== user.id) throw new Error("Invalid account profile response.");
      setProfile(result); setName(result.name);
    }).catch((failure: unknown) => {
      if (active) setError(safeAuthErrorMessage(failure, "general"));
    }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [user?.id, revision]);

  async function save(event: FormEvent) {
    event.preventDefault();
    const value = name.trim();
    if (!value || Array.from(value).length > 80 || /[\u0000-\u001f\u007f-\u009f]/u.test(value)) {
      setNotice("Enter a display name of 1–80 characters without control characters."); return;
    }
    if (!user || !profile || profile.id !== user.id) return;
    setSaving(true); setNotice(null);
    try {
      const updated = await accountUpdateProfile({ name: value });
      if (!isAccountProfile(updated) || updated.id !== user.id) throw new Error("Invalid account profile response.");
      setProfile(updated); setName(updated.name); setEditing(false); setNotice("Profile updated.");
      onProfileSaved();
    } catch (failure) { setNotice(safeAuthErrorMessage(failure, "general")); }
    finally { setSaving(false); }
  }

  const displayUser = profile && user ? { ...user, name: profile.name } : user;
  return <div className="page page--profile" data-testid="profile-page">
    <div className="profile-actions">
      <RouterLink to="chat" className="button button--ghost button--sm">← Back to chat</RouterLink>
      {profile && <button type="button" className="button button--ghost button--sm" onClick={() => { setEditing(!editing); setName(profile.name); setNotice(null); }}>{editing ? "Cancel" : "Edit"}</button>}
    </div>
    {!user ? <div className="profile-empty"><h1>Profile</h1><p>Sign in to see and edit your Nexora profile. Local chat remains available without an account.</p><button className="button button--primary" onClick={onRequestSignIn}>Sign in</button></div> : <>
      <header className="profile-header">
        {displayUser && <AccountAvatar user={displayUser} large />}
        <h1>{profile?.name || user.name || "Your profile"}</h1>
        <p>Private Nexora account</p>
      </header>
      {loading && <p role="status" className="profile-message">Loading profile…</p>}
      {error && <div role="alert" className="profile-message"><p>{error}</p><button className="button button--ghost button--sm" onClick={() => setRevision((n) => n + 1)}>Retry</button></div>}
      {notice && <p role="status" className="profile-message">{notice}</p>}
      {editing && profile && <form className="profile-edit" onSubmit={(event) => void save(event)}>
        <label className="field"><span className="field__label">Display name</span><input className="field__input" autoComplete="name" value={name} maxLength={160} disabled={saving} onChange={(event) => setName(event.target.value)} /></label>
        <button className="button button--primary" type="submit" disabled={saving}>{saving ? "Saving…" : "Save changes"}</button>
      </form>}
      {profile && <section className="profile-details" aria-label="Profile information"><h2>Profile information</h2><div><span>Display name</span><strong>{profile.name}</strong></div><div><span>Account since</span><strong>{Number.isFinite(Date.parse(profile.createdAt)) ? new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(Date.parse(profile.createdAt)) : "Not available"}</strong></div><p>Only you can view this page. Account security and email settings are managed separately.</p><RouterLink to="account" className="button button--ghost button--sm">Account & security</RouterLink></section>}
    </>}
  </div>;
}
