import fs from "node:fs";
import path from "node:path";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import type { BillingStatus } from "../lib/auth";
import { markDesktop } from "../test/fixtures";
import { PlansPage } from "./PlansPage";

function makePlanStatus(
  effectivePlanId: string,
  effectivePlanName: string,
): BillingStatus {
  return {
    effectivePlanId,
    effectivePlanName,
    subscription: null,
    recentOrders: [],
    paymentConfigured: false,
    portalAvailable: false,
  };
}

describe("Phase 14 — Plans and Premium overview", () => {
  beforeEach(() => {
    markDesktop(true);
    invokeMock.mockReset();
  });

  afterEach(() => {
    markDesktop(false);
  });

  it("keeps signed-out local chat available and leaves Premium terms and actions undecided", async () => {
    const onRequestSignIn = vi.fn();
    const user = userEvent.setup();
    render(
      <PlansPage
        isAuthenticated={false}
        accountId={null}
        onRequestSignIn={onRequestSignIn}
      />,
    );

    expect(screen.getByRole("heading", { name: "Plans & Premium" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Local workspace" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Nexora Premium" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Open AI Chat" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("No account is signed in");

    const undecidedValues = screen.getAllByText("Undecided");
    expect(undecidedValues).toHaveLength(4);
    const pageText = screen.getByTestId("plans-page").textContent ?? "";
    expect(pageText).not.toMatch(/\$|€|₹|\b\d+\s*(?:per month|per year|\/\s*(?:mo|yr))\b/i);
    expect(pageText).not.toMatch(/\b\d+\s*(?:messages|tokens|requests|generations)\b/i);
    expect(pageText).not.toMatch(/payment successful|subscription activated|purchase complete/i);

    const unavailableAction = screen.getByRole("button", {
      name: "Unavailable in this phase",
    }) as HTMLButtonElement;
    expect(unavailableAction.disabled).toBe(true);
    await user.click(unavailableAction);
    expect(onRequestSignIn).not.toHaveBeenCalled();
    expect(invokeMock).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Sign in to view account status" }));
    expect(onRequestSignIn).toHaveBeenCalledTimes(1);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("shows an account-plan record only from the authenticated account-status response", async () => {
    invokeMock.mockResolvedValueOnce(
      makePlanStatus("account-plan-42", "Account-reported plan"),
    );
    render(
      <PlansPage
        isAuthenticated={true}
        accountId="account-42"
        onRequestSignIn={vi.fn()}
      />,
    );

    const currentPlan = await screen.findByTestId("account-plan-record");
    expect(currentPlan.textContent).toContain("Account-reported plan");
    expect(currentPlan.textContent).toContain("does not verify");
    expect(currentPlan.querySelector("[data-plan-id='account-plan-42']")).toBeTruthy();
    expect(invokeMock.mock.calls.map(([command]) => command)).toEqual([
      "billing_get_status",
    ]);
    expect(invokeMock).not.toHaveBeenCalledWith("billing_list_plans");
    expect(invokeMock).not.toHaveBeenCalledWith("usage_get_server_summary");
    expect(invokeMock).not.toHaveBeenCalledWith("billing_create_checkout");
  });

  it("announces loading and then renders a reported account-plan record when account status resolves", async () => {
    let resolveStatus!: (status: BillingStatus) => void;
    invokeMock.mockImplementation(
      () => new Promise<BillingStatus>((resolve) => (resolveStatus = resolve)),
    );
    render(
      <PlansPage
        isAuthenticated={true}
        accountId="account-loading"
        onRequestSignIn={vi.fn()}
      />,
    );

    expect((await screen.findByRole("status")).textContent).toContain("Checking plan status");
    expect(screen.queryByTestId("account-plan-record")).toBeNull();
    await act(async () => {
      resolveStatus(makePlanStatus("verified-id", "Verified account plan"));
    });
    expect(await screen.findByText("Verified account plan")).toBeTruthy();
  });

  it("shows no current plan when status is incomplete instead of assuming Free", async () => {
    invokeMock.mockResolvedValueOnce({
      ...makePlanStatus("", "  "),
    });
    render(
      <PlansPage
        isAuthenticated={true}
        accountId="account-incomplete"
        onRequestSignIn={vi.fn()}
      />,
    );

    expect(
      await screen.findByText(/Plan status is unavailable\. No Free plan or subscription state is assumed\./),
    ).toBeTruthy();
    expect(screen.queryByTestId("account-plan-record")).toBeNull();
    expect(screen.queryByText(/^Free$/)).toBeNull();
  });

  it("reports an unconfigured status as unavailable without inventing an account plan", async () => {
    invokeMock.mockRejectedValueOnce({
      code: "backend_unconfigured",
      message: "Account service is not configured.",
    });
    render(
      <PlansPage
        isAuthenticated={true}
        accountId="account-unconfigured"
        onRequestSignIn={vi.fn()}
      />,
    );

    expect(
      await screen.findByText(/Plan status is unavailable\. No Free plan or subscription state is assumed\./),
    ).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByTestId("account-plan-record")).toBeNull();
  });

  it("shows a distinct error state when account status cannot be loaded", async () => {
    invokeMock.mockRejectedValueOnce({ code: "transport_error" });
    render(
      <PlansPage
        isAuthenticated={true}
        accountId="account-error"
        onRequestSignIn={vi.fn()}
      />,
    );

    expect(
      await screen.findByRole("alert"),
    ).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain(
      "No plan or subscription change was attempted",
    );
    expect(screen.queryByTestId("account-plan-record")).toBeNull();
  });

  it("discards a previous account response after an account switch", async () => {
    const pending: Array<{ resolve: (status: BillingStatus) => void }> = [];
    invokeMock.mockImplementation(
      () =>
        new Promise<BillingStatus>((resolve) => {
          pending.push({ resolve });
        }),
    );
    const view = render(
      <PlansPage
        isAuthenticated={true}
        accountId="account-a"
        onRequestSignIn={vi.fn()}
      />,
    );
    await waitFor(() => expect(pending).toHaveLength(1));

    view.rerender(
      <PlansPage
        isAuthenticated={true}
        accountId="account-b"
        onRequestSignIn={vi.fn()}
      />,
    );
    await waitFor(() => expect(pending).toHaveLength(2));

    await act(async () => {
      pending[0]?.resolve(makePlanStatus("plan-a", "Plan for account A"));
    });
    expect(screen.queryByText("Plan for account A")).toBeNull();
    expect(screen.getByRole("status").textContent).toContain("Checking plan status");

    await act(async () => {
      pending[1]?.resolve(makePlanStatus("plan-b", "Plan for account B"));
    });
    expect(await screen.findByText("Plan for account B")).toBeTruthy();
    expect(screen.queryByText("Plan for account A")).toBeNull();
  });

  it("clears account plan data immediately on sign-out and ignores the late response", async () => {
    let resolveStatus!: (status: BillingStatus) => void;
    invokeMock.mockImplementation(
      () => new Promise<BillingStatus>((resolve) => (resolveStatus = resolve)),
    );
    const view = render(
      <PlansPage
        isAuthenticated={true}
        accountId="account-before-sign-out"
        onRequestSignIn={vi.fn()}
      />,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    view.rerender(
      <PlansPage
        isAuthenticated={false}
        accountId={null}
        onRequestSignIn={vi.fn()}
      />,
    );
    expect(screen.getByRole("status").textContent).toContain("No account is signed in");
    expect(screen.queryByTestId("account-plan-record")).toBeNull();

    await act(async () => {
      resolveStatus(makePlanStatus("old-plan", "Stale signed-in plan"));
    });
    expect(screen.queryByText("Stale signed-in plan")).toBeNull();
    expect(screen.getByRole("status").textContent).toContain("No account is signed in");
  });

  it("uses Phase 07 theme tokens and collapses the comparison for narrow windows", () => {
    const appCss = fs.readFileSync(path.resolve(__dirname, "../styles/app.css"), "utf8");
    const tokensCss = fs.readFileSync(path.resolve(__dirname, "../styles/tokens.css"), "utf8");
    const start = appCss.indexOf("/* Phase 14 — restrained, read-only Plans & Premium overview */");
    const end = appCss.indexOf("/* =========================================================================", start);
    const plansCss = appCss.slice(start, end);

    expect(start).toBeGreaterThanOrEqual(0);
    expect(plansCss).toContain("var(--bg-panel)");
    expect(plansCss).toContain("var(--bg-elevated)");
    expect(plansCss).toContain("var(--text-primary)");
    expect(plansCss).toContain("grid-template-columns: repeat(2, minmax(0, 1fr));");
    expect(plansCss).toContain("@media (max-width: 760px)");
    expect(plansCss).toContain("grid-template-columns: minmax(0, 1fr);");
    expect(plansCss).toContain("min-width: 0;");
    expect(plansCss).toContain("@media (prefers-reduced-motion: reduce)");
    expect(tokensCss).toContain(':root[data-theme="dark"]');
    expect(tokensCss).toContain(':root[data-theme="light"]');
  });
});
