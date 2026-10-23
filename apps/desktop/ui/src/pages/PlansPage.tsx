import { useCallback, useEffect, useState } from "react";
import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";
import {
  billingCancelSubscription,
  billingChangePlan,
  billingCreateCheckout,
  billingGetStatus,
  billingListPlans,
  billingOpenPortal,
  billingVerifyOrder,
  usageGetServerSummary,
  type BillingStatus,
  type PlanCatalog,
  type PlanView,
  type ServerUsageSummary,
} from "../lib/auth";
import { PageHeader } from "./PageHeader";

interface PlansPageProps {
  isAuthenticated: boolean;
  onRequestSignIn: () => void;
}

function formatPrice(amountMinor: number, currency: string, interval: string): string {
  if (amountMinor === 0) return "$0";
  const major = (amountMinor / 100).toFixed(amountMinor % 100 === 0 ? 0 : 2);
  const symbol = currency.toLowerCase() === "usd" ? "$" : `${currency.toUpperCase()} `;
  return `${symbol}${major} / ${interval === "year" ? "yr" : "mo"}`;
}

export function PlansPage({ isAuthenticated, onRequestSignIn }: PlansPageProps) {
  const [catalog, setCatalog] = useState<PlanCatalog | null>(null);
  const [billing, setBilling] = useState<BillingStatus | null>(null);
  const [usage, setUsage] = useState<ServerUsageSummary | null>(null);
  const [interval, setIntervalState] = useState<"month" | "year">("month");
  const [loading, setLoading] = useState(true);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [notice, setNotice] = useState<{ tone: "info" | "success" | "error"; text: string } | null>(
    null,
  );

  const loadAll = useCallback(async () => {
    setLoading(true);
    try {
      const plansRes = await billingListPlans();
      setCatalog(plansRes && Array.isArray(plansRes.plans) ? plansRes : null);
      if (isAuthenticated) {
        const [billRes, usageRes] = await Promise.all([
          billingGetStatus().catch(() => null),
          usageGetServerSummary().catch(() => null),
        ]);
        setBilling(billRes);
        setUsage(usageRes);
      } else {
        setBilling(null);
        setUsage(null);
      }
    } catch (err: unknown) {
      setCatalog(null);
      const msg =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : "Plan catalogue is unavailable until a Nexora account service is configured.";
      setNotice({ tone: "info", text: msg });
    } finally {
      setLoading(false);
    }
  }, [isAuthenticated]);

  useEffect(() => {
    void loadAll();
  }, [loadAll]);

  const effectivePlan = billing?.effectivePlanId ?? "free";

  const handleSelectPlan = async (plan: PlanView) => {
    if (!isAuthenticated) {
      onRequestSignIn();
      return;
    }
    setNotice(null);
    setBusyAction(plan.id);
    try {
      if (billing?.subscription && billing.subscription.status === "active") {
        const updated = await billingChangePlan(plan.id, interval);
        setBilling(updated);
        const refreshedUsage = await usageGetServerSummary().catch(() => null);
        if (refreshedUsage) setUsage(refreshedUsage);
        setNotice({
          tone: "success",
          text: `Subscription updated to ${updated.effectivePlanName}.`,
        });
      } else {
        const key = `chk-${plan.id}-${interval}-${Date.now()}`;
        const session = await billingCreateCheckout({
          planId: plan.id,
          interval,
          idempotencyKey: key,
        });
        setNotice({
          tone: "info",
          text: `Checkout opened in your browser (Order ${session.orderId.slice(0, 8)}…). Click "Verify latest order" after completing payment.`,
        });
        await loadAll();
      }
    } catch (err: unknown) {
      const msg =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : "Billing action could not be completed.";
      setNotice({ tone: "error", text: msg });
    } finally {
      setBusyAction(null);
    }
  };

  const handleVerifyOrder = async (orderId: string) => {
    setBusyAction(`verify-${orderId}`);
    setNotice(null);
    try {
      const updated = await billingVerifyOrder(orderId);
      setBilling(updated);
      const refreshedUsage = await usageGetServerSummary().catch(() => null);
      if (refreshedUsage) setUsage(refreshedUsage);
      const matchedOrder = updated.recentOrders.find((o) => o.id === orderId);
      if (matchedOrder?.status === "completed") {
        setNotice({
          tone: "success",
          text: `Payment verified — active plan is now ${updated.effectivePlanName}.`,
        });
      } else {
        setNotice({
          tone: "info",
          text: `Payment has not completed (${matchedOrder?.status ?? "pending"}) — active plan remains ${updated.effectivePlanName}.`,
        });
      }
    } catch (err: unknown) {
      const msg =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : "Order verification failed.";
      setNotice({ tone: "error", text: msg });
    } finally {
      setBusyAction(null);
    }
  };

  const handleCancel = async (immediate: boolean) => {
    setBusyAction("cancel");
    setNotice(null);
    try {
      const updated = await billingCancelSubscription(immediate);
      setBilling(updated);
      setNotice({
        tone: "info",
        text: immediate
          ? "Subscription cancelled immediately. Plan reverted to Free."
          : "Subscription set to cancel at the end of the current billing period.",
      });
    } catch (err: unknown) {
      const msg =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : "Could not cancel subscription.";
      setNotice({ tone: "error", text: msg });
    } finally {
      setBusyAction(null);
    }
  };

  const handleOpenPortal = async () => {
    setBusyAction("portal");
    setNotice(null);
    try {
      await billingOpenPortal();
      setNotice({ tone: "info", text: "Opened billing portal in your browser." });
    } catch (err: unknown) {
      const msg =
        typeof err === "object" && err !== null && "message" in err
          ? String((err as { message: unknown }).message)
          : "Billing portal unavailable.";
      setNotice({ tone: "error", text: msg });
    } finally {
      setBusyAction(null);
    }
  };

  const serverPlans = catalog?.plans ?? [];

  return (
    <div className="page page--plans" data-testid="plans-page">
      <PageHeader
        eyebrow="Plans"
        title="Plans & Billing"
        copy="All model execution runs locally on your machine. Plan tiers and billing options are loaded directly from your configured Nexora account server without hardcoded client pricing."
      />

      <div className="page-stack">
        {notice && (
          <div className={`status status--${notice.tone}`} role="status">
            {notice.text}
          </div>
        )}

        {catalog && !catalog.paymentConfigured && (
          <div className="plans-banner" role="note">
            <Icon name="alert" size={14} />
            <span>
              Billing checkout is not active on this server yet. Displayed tiers reflect server-reported configuration only.
            </span>
          </div>
        )}

        {isAuthenticated && (
          <section className="settings-group" aria-label="Active subscription and daily usage">
            <div className="plans-header-row">
              <div>
                <h2 className="settings-group__title">Current Plan &amp; Today’s Model Usage</h2>
                <p className="settings-row__hint">
                  Active plan: <strong>{billing?.effectivePlanName ?? usage?.planName ?? "Free"}</strong>
                  {usage ? ` · Resets at ${usage.resetsAtUtc}` : ""}
                </p>
              </div>
              <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                <button
                  type="button"
                  className="button button--ghost button--sm"
                  onClick={() => void loadAll()}
                  disabled={loading}
                >
                  <Icon name="refresh" size={13} /> Refresh
                </button>
                {billing?.portalAvailable && (
                  <button
                    type="button"
                    className="button button--ghost button--sm"
                    onClick={() => void handleOpenPortal()}
                    disabled={busyAction === "portal"}
                  >
                    Manage billing portal
                  </button>
                )}
              </div>
            </div>

            {usage && (
              <div className="plans-quota-grid">
                {usage.models.map((m) => {
                  const pct =
                    m.dailyLimit > 0
                      ? Math.min(100, Math.round(((m.used + m.reserved) / m.dailyLimit) * 100))
                      : 0;
                  return (
                    <div key={m.modelId} className="plans-quota-card">
                      <div className="plans-quota-card__top">
                        <strong>
                          {m.modelId === "nexora-sonnet" ? "Nexora Sonnet (14B)" : "Nexora Mini (7B)"}
                        </strong>
                        <span>
                          {m.remaining} of {m.dailyLimit} remaining today
                        </span>
                      </div>
                      <div className="plans-quota-bar" aria-hidden="true">
                        <div className="plans-quota-bar__fill" style={{ width: `${pct}%` }} />
                      </div>
                    </div>
                  );
                })}
              </div>
            )}

            {billing?.subscription && billing.subscription.planId !== "free" && (
              <div className="settings-row settings-row--static" style={{ marginTop: 12 }}>
                <span>
                  <span className="settings-row__label">
                    Subscription status: {billing.subscription.status}
                  </span>
                  <span className="settings-row__hint">
                    Current period ends {billing.subscription.currentPeriodEnd}
                    {billing.subscription.cancelAtPeriodEnd ? " (cancels at period end)" : ""}
                  </span>
                </span>
                <div style={{ display: "flex", gap: 8 }}>
                  {!billing.subscription.cancelAtPeriodEnd && (
                    <button
                      type="button"
                      className="button button--ghost button--sm"
                      disabled={busyAction === "cancel"}
                      onClick={() => void handleCancel(false)}
                    >
                      Cancel at period end
                    </button>
                  )}
                  <button
                    type="button"
                    className="button button--ghost button--sm"
                    disabled={busyAction === "cancel"}
                    onClick={() => void handleCancel(true)}
                  >
                    Cancel immediately
                  </button>
                </div>
              </div>
            )}

            {billing && billing.recentOrders.length > 0 && (
              <div className="settings-row settings-row--static">
                <span>
                  <span className="settings-row__label">
                    Latest order ({billing.recentOrders[0]!.planId.toUpperCase()} —{" "}
                    {billing.recentOrders[0]!.status})
                  </span>
                  <span className="settings-row__hint">
                    Order ID: {billing.recentOrders[0]!.id}
                  </span>
                </span>
                {billing.recentOrders[0]!.status === "pending" && (
                  <button
                    type="button"
                    className="button button--primary button--sm"
                    disabled={busyAction !== null}
                    onClick={() => void handleVerifyOrder(billing.recentOrders[0]!.id)}
                  >
                    Verify latest order
                  </button>
                )}
              </div>
            )}
          </section>
        )}

        <section className="settings-group" aria-label="Available plans">
          <div className="plans-header-row">
            <h2 className="settings-group__title">Available Plans</h2>
            {serverPlans.length > 0 && (
              <div className="plans-interval-toggle" role="group" aria-label="Billing interval">
                <button
                  type="button"
                  className={`plans-interval-btn${interval === "month" ? " plans-interval-btn--active" : ""}`}
                  onClick={() => setIntervalState("month")}
                >
                  Monthly
                </button>
                <button
                  type="button"
                  className={`plans-interval-btn${interval === "year" ? " plans-interval-btn--active" : ""}`}
                  onClick={() => setIntervalState("year")}
                >
                  Yearly
                </button>
              </div>
            )}
          </div>

          {loading && !catalog ? (
            <p className="settings-row__hint">Loading plan catalogue…</p>
          ) : serverPlans.length === 0 ? (
            <div className="plans-unconfigured" data-testid="plans-unconfigured-state">
              <p className="plans-unconfigured__title">
                Commercial plan catalogue is not connected in this local build
              </p>
              <p className="plans-unconfigured__body">
                Nexora never invents subscription prices, quotas, or active billing status in the client. Local chat and workspace features remain available right now on this device without signing in.
              </p>
              <div className="plans-unconfigured__actions">
                <RouterLink to="chat" className="button button--primary button--sm">
                  Back to AI Chat
                </RouterLink>
                {!isAuthenticated && (
                  <button
                    type="button"
                    className="button button--ghost button--sm"
                    onClick={onRequestSignIn}
                  >
                    Sign in
                  </button>
                )}
                <button
                  type="button"
                  className="button button--ghost button--sm"
                  onClick={() => void loadAll()}
                >
                  Retry catalogue check
                </button>
              </div>
            </div>
          ) : (
            <div className="plans-cards-grid">
              {serverPlans.map((plan) => {
                const price =
                  plan.prices.find((p) => p.interval === interval) ?? plan.prices[0] ?? null;
                const isCurrent = effectivePlan === plan.id && isAuthenticated;
                return (
                  <article
                    key={plan.id}
                    className={`plan-card${isCurrent ? " plan-card--current" : ""}`}
                    data-testid={`plan-card-${plan.id}`}
                  >
                    <div className="plan-card__header">
                      <h3 className="plan-card__name">{plan.name}</h3>
                      {isCurrent && <span className="badge badge--local">Current plan</span>}
                    </div>
                    <div className="plan-card__price">
                      {price
                        ? formatPrice(price.amountMinor, price.currency, price.interval)
                        : "Pricing unavailable"}
                    </div>
                    <p className="plan-card__desc">{plan.description}</p>
                    <ul className="plan-card__features">
                      {plan.features.map((feat) => (
                        <li key={feat}>
                          <Icon name="check" size={13} />
                          <span>{feat}</span>
                        </li>
                      ))}
                    </ul>
                    <div className="plan-card__actions">
                      {plan.id === "free" ? (
                        <button
                          type="button"
                          className="button button--ghost button--sm"
                          disabled={isCurrent || !isAuthenticated || busyAction !== null}
                          onClick={() => void handleSelectPlan(plan)}
                        >
                          {isCurrent
                            ? "Active"
                            : isAuthenticated
                              ? "Downgrade to Free"
                              : "Included with account"}
                        </button>
                      ) : (
                        <button
                          type="button"
                          className="button button--primary button--sm"
                          disabled={isCurrent || busyAction !== null}
                          onClick={() => void handleSelectPlan(plan)}
                        >
                          {busyAction === plan.id
                            ? "Processing…"
                            : isCurrent
                              ? "Active plan"
                              : isAuthenticated
                                ? `Upgrade to ${plan.name}`
                                : `Sign in for ${plan.name}`}
                        </button>
                      )}
                    </div>
                  </article>
                );
              })}
            </div>
          )}

          <div className="settings-row settings-row--static" style={{ marginTop: 16 }}>
            <span>
              <span className="settings-row__label">Air-gapped offline licence keys</span>
              <span className="settings-row__hint">
                Have a cryptographically signed offline key? Manage offline verification locally without network access.
              </span>
            </span>
            <RouterLink to="licensing" className="button button--ghost button--sm">
              Offline Licence Key
            </RouterLink>
          </div>
        </section>
      </div>
    </div>
  );
}
