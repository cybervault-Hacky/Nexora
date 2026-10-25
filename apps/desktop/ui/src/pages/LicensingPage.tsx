import { useState, type FormEvent } from "react";
import { useLicence } from "../features/licensing/useLicence";
import { PageHeader } from "./PageHeader";

export function LicensingPage() {
  const { licence, loading, activateKey, deactivateKey } = useLicence();
  const [inputKey, setInputKey] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [notice, setNotice] = useState<{ tone: "success" | "error"; message: string } | null>(null);

  const handleActivate = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (submitting || !inputKey.trim()) return;

    setNotice(null);
    setSubmitting(true);
    try {
      const res = await activateKey(inputKey.trim());
      setInputKey("");
      setNotice({
        tone: "success",
        message: `Successfully activated ${res.planName}!`,
      });
    } catch (err: unknown) {
      const msg = typeof err === "object" && err !== null && "message" in err
        ? String((err as { message: unknown }).message)
        : "Licence key validation failed. Check your key and try again.";
      setNotice({ tone: "error", message: msg });
    } finally {
      setSubmitting(false);
    }
  };

  const handleDeactivate = async () => {
    if (submitting) return;
    setSubmitting(true);
    setNotice(null);
    try {
      await deactivateKey();
      setNotice({
        tone: "success",
        message: "Licence deactivated. Reverted to Community Edition.",
      });
    } catch {
      setNotice({ tone: "error", message: "Failed to deactivate licence." });
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="page page--licensing">
      <PageHeader
        eyebrow="Offline Licence Keys"
        title="Developer Licensing"
        copy="Manage a locally verified offline licence key. Licence activation and status are separate from account plans; commercial terms are not established here."
      />

      <div className="page-stack">
        {/* Active Licence Status */}
        <section className="settings-group" aria-label="Current Licence Status">
          <h2 className="settings-group__title">Current Licence Status</h2>

          {loading ? (
            <div className="settings-row settings-row--static">
              <span>Checking active licence…</span>
            </div>
          ) : (
            <>
              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">Active Edition</span>
                  <span className="settings-row__hint">{licence.planName}</span>
                </span>
                <span
                  className="badge badge--success"
                  style={{
                    textTransform: "uppercase",
                    fontWeight: 700,
                    letterSpacing: "0.05em",
                  }}
                >
                  {licence.tier}
                </span>
              </div>

              {licence.tier !== "community" && licence.licensee && (
                <div className="settings-row settings-row--static">
                  <span>
                    <span className="settings-row__label">Licensed Developer / Org</span>
                    <span className="settings-row__hint">{licence.licensee}</span>
                  </span>
                </div>
              )}

              {licence.tier !== "community" && (
                <div className="settings-row settings-row--static">
                  <span>
                    <span className="settings-row__label">Licence Validity</span>
                    <span className="settings-row__hint">
                      {licence.expiresAt
                        ? `Valid until ${new Date(licence.expiresAt * 1000).toLocaleDateString()}`
                        : "No expiry date recorded"}
                    </span>
                  </span>
                  <button
                    type="button"
                    className="button button--ghost button--sm"
                    disabled={submitting}
                    onClick={handleDeactivate}
                  >
                    Deactivate licence
                  </button>
                </div>
              )}

              {licence.tier === "community" && (
                <div className="settings-row settings-row--static">
                  <span>
                    <span className="settings-row__label">Offline licence status</span>
                    <span className="settings-row__hint">
                      This locally reported status is separate from account plans and does not define
                      commercial terms.
                    </span>
                  </span>
                  <span className="badge badge--local">Local licence</span>
                </div>
              )}
            </>
          )}
        </section>

        {/* Licence Activation Form */}
        <section className="settings-group" aria-label="Activate Licence Key">
          <h2 className="settings-group__title">Activate Offline Licence</h2>
          <form onSubmit={handleActivate} style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            {notice && (
              <div className={`status status--${notice.tone}`} role="status">
                {notice.message}
              </div>
            )}

            <div className="settings-row settings-row--static" style={{ flexDirection: "column", alignItems: "flex-start", gap: 6 }}>
              <label htmlFor="licence-key-input" className="settings-row__label">
                Licence Key
              </label>
              <input
                id="licence-key-input"
                type="text"
                className="input"
                style={{ width: "100%", maxWidth: 480, fontFamily: "monospace" }}
                placeholder="Enter an issued offline licence key"
                value={inputKey}
                onChange={(e) => setInputKey(e.target.value)}
                disabled={submitting}
                autoComplete="off"
                spellCheck="false"
              />
              <span className="settings-row__hint">
                Licence keys are cryptographically verified locally. Verification requires zero network calls and works in air-gapped environments.
              </span>
            </div>

            <div style={{ display: "flex", gap: 12 }}>
              <button
                type="submit"
                className="button button--primary button--sm"
                disabled={submitting || !inputKey.trim()}
              >
                {submitting ? "Validating…" : "Activate key"}
              </button>
            </div>
          </form>
        </section>


      </div>
    </div>
  );
}
