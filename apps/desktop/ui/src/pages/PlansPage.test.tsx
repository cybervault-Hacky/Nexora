import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { PlansPage } from "./PlansPage";
import { markDesktop } from "../test/fixtures";

describe("PlansPage — Server Catalogue, Daily Per-Model Quotas & Checkout Lifecycle", () => {
  beforeEach(() => {
    markDesktop(true);
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "billing_list_plans") {
        return {
          paymentConfigured: true,
          provider: "stripe",
          plans: [
            {
              id: "free",
              name: "Free",
              description: "Daily local AI coding assistance.",
              features: ["25 Nexora Mini messages/day", "5 Nexora Sonnet messages/day"],
              modelQuotas: { "nexora-mini": 25, "nexora-sonnet": 5 },
              prices: [{ id: "free_mo", planId: "free", interval: "month", currency: "usd", amountMinor: 0 }],
            },
            {
              id: "pro",
              name: "Pro",
              description: "Expanded daily quotas for solo engineers.",
              features: ["300 Nexora Mini messages/day", "100 Nexora Sonnet messages/day"],
              modelQuotas: { "nexora-mini": 300, "nexora-sonnet": 100 },
              prices: [
                { id: "pro_mo", planId: "pro", interval: "month", currency: "usd", amountMinor: 1900 },
                { id: "pro_yr", planId: "pro", interval: "year", currency: "usd", amountMinor: 19000 },
              ],
            },
            {
              id: "team",
              name: "Team",
              description: "High-volume daily quotas for teams.",
              features: ["1,000 Nexora Mini messages/day", "400 Nexora Sonnet messages/day"],
              modelQuotas: { "nexora-mini": 1000, "nexora-sonnet": 400 },
              prices: [
                { id: "team_mo", planId: "team", interval: "month", currency: "usd", amountMinor: 3900 },
                { id: "team_yr", planId: "team", interval: "year", currency: "usd", amountMinor: 39000 },
              ],
            },
          ],
        };
      }
      if (cmd === "billing_get_status") {
        return {
          effectivePlanId: "free",
          effectivePlanName: "Free",
          subscription: null,
          recentOrders: [],
          paymentConfigured: true,
          portalAvailable: false,
        };
      }
      if (cmd === "usage_get_server_summary") {
        return {
          planId: "free",
          planName: "Free",
          periodDayUtc: "2026-10-22",
          resetsAtUtc: "2026-10-23T00:00:00Z",
          models: [
            { modelId: "nexora-mini", dailyLimit: 25, used: 3, reserved: 0, remaining: 22 },
            { modelId: "nexora-sonnet", dailyLimit: 5, used: 1, reserved: 0, remaining: 4 },
          ],
        };
      }
      if (cmd === "billing_create_checkout") {
        return {
          orderId: "ord-pro-12345678",
          checkoutUrl: "https://checkout.stripe.test/pay/cs_123",
          providerSessionId: "cs_123",
          expiresNote: "Redirecting to checkout",
        };
      }
      return undefined;
    });
  });

  it("renders Free, Pro, and Team plans, switches Monthly/Yearly pricing, displays live per-model daily usage, and initiates server-priced checkout", async () => {
    const user = userEvent.setup();
    render(<PlansPage isAuthenticated={true} onRequestSignIn={vi.fn()} />);

    expect(await screen.findByTestId("plan-card-free")).toBeTruthy();
    expect(screen.getByTestId("plan-card-pro")).toBeTruthy();
    expect(screen.getByTestId("plan-card-team")).toBeTruthy();
    expect(screen.getByText(/22 of 25 remaining today/i)).toBeTruthy();
    expect(screen.getByText(/4 of 5 remaining today/i)).toBeTruthy();
    expect(screen.getByText("$19 / mo")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Yearly" }));
    expect(screen.getByText("$190 / yr")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Upgrade to Pro" }));
    expect(invokeMock).toHaveBeenCalledWith(
      "billing_create_checkout",
      expect.objectContaining({
        request: expect.objectContaining({ planId: "pro", interval: "year" }),
      }),
    );
  });

  it("triggers sign-in modal when an unauthenticated user clicks Upgrade", async () => {
    const onRequestSignIn = vi.fn();
    const user = userEvent.setup();
    render(<PlansPage isAuthenticated={false} onRequestSignIn={onRequestSignIn} />);

    await user.click(await screen.findByRole("button", { name: "Sign in for Pro" }));
    expect(onRequestSignIn).toHaveBeenCalledTimes(1);
  });

  it("keeps plan on Free and displays honest pending status when order verification has not completed payment", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "billing_list_plans") {
        return {
          paymentConfigured: true,
          provider: "stripe",
          plans: [
            {
              id: "free",
              name: "Free",
              description: "Starter daily local coding quotas.",
              features: ["25 Nexora Mini messages/day"],
              modelQuotas: { "nexora-mini": 25, "nexora-sonnet": 5 },
              prices: [{ id: "free_mo", planId: "free", interval: "month", currency: "usd", amountMinor: 0 }],
            },
          ],
        };
      }
      if (cmd === "billing_get_status" || cmd === "billing_verify_order") {
        return {
          effectivePlanId: "free",
          effectivePlanName: "Free",
          subscription: null,
          recentOrders: [
            {
              id: "ord-pending-0001",
              planId: "pro",
              amountMinor: 1900,
              currency: "USD",
              status: "pending",
              createdAt: "2026-10-22T12:00:00Z",
            },
          ],
          paymentConfigured: true,
          portalAvailable: false,
        };
      }
      if (cmd === "usage_get_server_summary") {
        return {
          planId: "free",
          planName: "Free",
          periodDayUtc: "2026-10-22",
          resetsAtUtc: "2026-10-23T00:00:00Z",
          models: [{ modelId: "nexora-mini", dailyLimit: 25, used: 0, reserved: 0, remaining: 25 }],
        };
      }
      return undefined;
    });

    const user = userEvent.setup();
    render(<PlansPage isAuthenticated={true} onRequestSignIn={vi.fn()} />);

    await user.click(await screen.findByRole("button", { name: /Verify/i }));
    expect(await screen.findByText(/Payment has not completed \(pending\) — active plan remains Free/i)).toBeTruthy();
  });
});
