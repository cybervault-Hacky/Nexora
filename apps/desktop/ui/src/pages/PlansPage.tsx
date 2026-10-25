import { useEffect, useState } from "react";
import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";
import { billingGetStatus, type BillingStatus } from "../lib/auth";
import { PageHeader } from "./PageHeader";

interface PlansPageProps {
  isAuthenticated: boolean;
  /** Used only to discard plan status from a previous signed-in account. */
  accountId: string | null;
  onRequestSignIn: () => void;
}

type AccountPlanLookup =
  | { kind: "idle" }
  | { kind: "signed-out" }
  | { kind: "loading"; accountId: string }
  | { kind: "available"; accountId: string; planId: string; planName: string }
  | { kind: "unavailable"; accountId: string | null }
  | { kind: "error"; accountId: string };

function readCurrentPlan(value: unknown): Pick<BillingStatus, "effectivePlanId" | "effectivePlanName"> | null {
  if (typeof value !== "object" || value === null) return null;
  const response = value as Record<string, unknown>;
  const planId = response.effectivePlanId;
  const planName = response.effectivePlanName;
  if (
    typeof planId !== "string" || planId.trim().length === 0 ||
    typeof planName !== "string" || planName.trim().length === 0
  ) {
    return null;
  }
  return { effectivePlanId: planId.trim(), effectivePlanName: planName.trim() };
}

function errorCode(error: unknown): string | null {
  if (typeof error !== "object" || error === null || !("code" in error)) return null;
  return typeof error.code === "string" ? error.code : null;
}

function isPlanStatusUnavailable(error: unknown): boolean {
  return ["backend_unconfigured", "desktop_required", "unauthenticated", "session_expired"].includes(
    errorCode(error) ?? "",
  );
}

export function PlansPage({ isAuthenticated, accountId, onRequestSignIn }: PlansPageProps) {
  const [lookup, setLookup] = useState<AccountPlanLookup>({ kind: "idle" });
  const [refreshKey, setRefreshKey] = useState(0);
  const normalizedAccountId = accountId?.trim() || null;

  useEffect(() => {
    let active = true;

    if (!isAuthenticated) {
      setLookup({ kind: "signed-out" });
      return () => {
        active = false;
      };
    }

    if (!normalizedAccountId) {
      setLookup({ kind: "unavailable", accountId: null });
      return () => {
        active = false;
      };
    }

    setLookup({ kind: "loading", accountId: normalizedAccountId });
    void billingGetStatus()
      .then((response) => {
        if (!active) return;
        const currentPlan = readCurrentPlan(response);
        setLookup(
          currentPlan
            ? {
                kind: "available",
                accountId: normalizedAccountId,
                planId: currentPlan.effectivePlanId,
                planName: currentPlan.effectivePlanName,
              }
            : { kind: "unavailable", accountId: normalizedAccountId },
        );
      })
      .catch((error: unknown) => {
        if (!active) return;
        setLookup(
          isPlanStatusUnavailable(error)
            ? { kind: "unavailable", accountId: normalizedAccountId }
            : { kind: "error", accountId: normalizedAccountId },
        );
      });

    return () => {
      active = false;
    };
  }, [isAuthenticated, normalizedAccountId, refreshKey]);

  // Never render a previous account's answer during the render before the new
  // account-specific request effect runs.
  const currentLookup: AccountPlanLookup = !isAuthenticated
    ? { kind: "signed-out" }
    : !normalizedAccountId
      ? { kind: "unavailable", accountId: null }
      : "accountId" in lookup && lookup.accountId === normalizedAccountId
        ? lookup
        : { kind: "loading", accountId: normalizedAccountId };

  const canRefresh =
    isAuthenticated &&
    normalizedAccountId !== null &&
    currentLookup.kind !== "loading";

  return (
    <div className="page page--plans" data-testid="plans-page">
      <PageHeader
        eyebrow="Plans"
        title="Nexora memberships"
        copy="Nexora is local-first. Local chat and the desktop workspace remain available while signed out. Premium is a planned concept: its capabilities and commercial terms are undecided, and no subscription actions are available here."
      />

      <div className="page-stack plans-page-stack">
        <section className="plans-account-panel" aria-labelledby="plans-account-heading">
          <div className="plans-account-panel__header">
            <div>
              <p className="eyebrow">Account</p>
              <h2 id="plans-account-heading" className="plans-section-title">
                Account plan record
              </h2>
            </div>
            {canRefresh && (
              <button
                type="button"
                className="button button--ghost button--sm"
                onClick={() => setRefreshKey((current) => current + 1)}
                aria-label="Refresh account plan status"
              >
                <Icon name="refresh" size={13} />
                Refresh status
              </button>
            )}
          </div>

          {currentLookup.kind === "signed-out" ? (
            <div className="plans-account-state">
              <p role="status" className="plans-account-state__copy">
                No account is signed in, so no account plan is shown. Sign-in is optional; local chat
                remains available without it.
              </p>
              <button
                type="button"
                className="button button--ghost button--sm"
                onClick={onRequestSignIn}
              >
                Sign in to view account status
              </button>
            </div>
          ) : currentLookup.kind === "loading" ? (
            <p role="status" className="plans-account-state__copy">
              Checking plan status with the signed-in account service…
            </p>
          ) : currentLookup.kind === "available" ? (
            <div className="plans-current-plan" data-testid="account-plan-record">
              <span className="badge badge--local">Account record</span>
              <strong className="plans-current-plan__name" data-plan-id={currentLookup.planId}>
                {currentLookup.planName}
              </strong>
              <p className="plans-account-state__copy">
                Reported by the authenticated account service only. This record does not verify
                provider payment, Premium access, or quota entitlement; this page cannot change them.
              </p>
            </div>
          ) : currentLookup.kind === "unavailable" ? (
            <p role="status" className="plans-account-state__copy">
              Plan status is unavailable. No Free plan or subscription state is assumed.
            </p>
          ) : (
            <p role="alert" className="plans-account-state__copy plans-account-state__copy--error">
              Could not load the current plan status. No plan or subscription change was attempted.
            </p>
          )}
        </section>

        <section className="plans-comparison" aria-labelledby="plans-comparison-heading">
          <div className="plans-section-heading">
            <h2 id="plans-comparison-heading" className="plans-section-title">
              Plan overview
            </h2>
            <p className="plans-section-copy">
              Available desktop functionality is separated from the unfinalized Premium concept.
              These descriptions are informational and do not grant entitlements.
            </p>
          </div>

          <div className="plans-tier-grid" role="group" aria-label="Plan comparison">
            <article className="plans-tier-card plans-tier-card--available">
              <div className="plans-tier-card__header">
                <div>
                  <p className="plans-tier-card__eyebrow">Available now · No billing required</p>
                  <h3 className="plans-tier-card__title">Free</h3>
                </div>
                <span className="badge badge--local">Local access</span>
              </div>
              <p className="plans-tier-card__price">No charge <small>for local use</small></p>
              <p className="plans-tier-card__description">
                The existing Nexora desktop experience on this device. Local chat remains available
                while signed out.
              </p>
              <ul className="plans-feature-list">
                <li>
                  <span className="plans-feature-list__icon" aria-hidden="true">
                    <Icon name="check" size={14} />
                  </span>
                  <span>Local chat and model selection</span>
                </li>
                <li>
                  <span className="plans-feature-list__icon" aria-hidden="true">
                    <Icon name="check" size={14} />
                  </span>
                  <span>Desktop workspace and code tools</span>
                </li>
                <li>
                  <span className="plans-feature-list__icon" aria-hidden="true">
                    <Icon name="check" size={14} />
                  </span>
                  <span>Local chat remains accessible while signed out</span>
                </li>
              </ul>
              <div className="plans-tier-card__action">
                <RouterLink to="chat" className="button button--primary button--sm">
                  Open AI Chat
                </RouterLink>
              </div>
            </article>

            <article className="plans-tier-card plans-tier-card--planned">
              <div className="plans-tier-card__header">
                <div>
                  <p className="plans-tier-card__eyebrow">Planned concept</p>
                  <h3 className="plans-tier-card__title">Nexora Premium</h3>
                </div>
                <span className="badge">Coming soon</span>
              </div>
              <p className="plans-tier-card__price">Coming soon <small>pricing and billing not finalized</small></p>
              <p className="plans-tier-card__description">
                A future plan concept only. No Premium offer or included feature set is currently
                available from this page.
              </p>
              <dl className="plans-undecided-list">
                <div>
                  <dt>Capabilities and eligibility</dt>
                  <dd>Undecided</dd>
                </div>
                <div>
                  <dt>Price, currency and billing interval</dt>
                  <dd>Undecided</dd>
                </div>
                <div>
                  <dt>Usage allowances and model access</dt>
                  <dd>Undecided</dd>
                </div>
                <div>
                  <dt>Other commercial terms</dt>
                  <dd>Undecided</dd>
                </div>
              </dl>
              <div className="plans-tier-card__action plans-tier-card__action--unavailable">
                <button
                  type="button"
                  className="button button--ghost button--sm"
                  disabled
                  aria-describedby="plans-premium-unavailable-note"
                >
                  Coming soon
                </button>
                <p id="plans-premium-unavailable-note" className="plans-action-note">
                  No checkout, payment, plan-change, or cancellation action is active here.
                </p>
              </div>
            </article>
          </div>
        </section>
      </div>
    </div>
  );
}
