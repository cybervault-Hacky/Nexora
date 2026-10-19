import { useState, type FormEvent } from "react";
import { Icon } from "../components/Icon";
import { useLicence } from "../features/licensing/useLicence";
import { PLAN_TIERS } from "../lib/licensing";
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
        eyebrow="Plans & Monetization"
        title="Developer Licensing"
        copy="Nexora is built on an offline-first sovereign foundation. Local model inference, code intelligence, and patch generation remain completely free forever on the Community Edition without requiring any licence."
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
                        : "Perpetual Offline Licence"}
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
                    <span className="settings-row__label">Community Sovereignty Guarantee</span>
                    <span className="settings-row__hint">
                      Your workspace is running in Community mode with 100% full local model execution,
                      patch generation, and code analysis. No feature gating, telemetry, or expiration.
                    </span>
                  </span>
                  <span className="badge badge--local">Offline Sovereign</span>
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
                placeholder="NEXORA-PRO-PERPETUAL_YOURNAME-XXXX"
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

        {/* Plan Comparison Grid */}
        <section className="settings-group" aria-label="Plan Comparison">
          <h2 className="settings-group__title">Edition Comparison</h2>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))",
              gap: 16,
              marginTop: 12,
            }}
          >
            {PLAN_TIERS.map((plan) => {
              const isCurrent = licence.tier === plan.id;
              return (
                <div
                  key={plan.id}
                  style={{
                    background: isCurrent
                      ? "var(--bg-elevated, rgba(139, 92, 246, 0.08))"
                      : "var(--bg-secondary, rgba(255, 255, 255, 0.02))",
                    border: isCurrent
                      ? "1px solid var(--accent, #8b5cf6)"
                      : "1px solid var(--border-subtle, rgba(255, 255, 255, 0.08))",
                    borderRadius: 8,
                    padding: 16,
                    display: "flex",
                    flexDirection: "column",
                    gap: 12,
                  }}
                >
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <h3 style={{ margin: 0, fontSize: 16, fontWeight: 600 }}>{plan.name}</h3>
                    {isCurrent ? (
                      <span className="badge badge--success" style={{ fontSize: 11 }}>
                        Current
                      </span>
                    ) : (
                      <span className="badge badge--local" style={{ fontSize: 11 }}>
                        {plan.badge}
                      </span>
                    )}
                  </div>

                  <p style={{ margin: 0, fontSize: 12, color: "var(--text-secondary)" }}>
                    {plan.tagline}
                  </p>

                  <div style={{ margin: "4px 0" }}>
                    <span style={{ fontSize: 24, fontWeight: 700 }}>{plan.price}</span>
                    <span style={{ fontSize: 12, color: "var(--text-tertiary)", marginLeft: 6 }}>
                      {plan.cadence}
                    </span>
                  </div>

                  <ul style={{ listStyle: "none", padding: 0, margin: 0, display: "flex", flexDirection: "column", gap: 8 }}>
                    {plan.features.map((feat, i) => (
                      <li
                        key={i}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 8,
                          fontSize: 12,
                          color: feat.included ? "var(--text-primary)" : "var(--text-tertiary)",
                          opacity: feat.included ? 1 : 0.5,
                        }}
                      >
                        <Icon name={feat.included ? "check" : "circle"} size={12} />
                        <span>{feat.name}</span>
                      </li>
                    ))}
                  </ul>
                </div>
              );
            })}
          </div>
        </section>
      </div>
    </div>
  );
}
