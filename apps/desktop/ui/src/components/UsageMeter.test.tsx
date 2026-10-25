import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { UsageMeter } from "./UsageMeter";
import { makeUsage } from "../test/fixtures";
import type { UsageView } from "../features/usage/useUsage";

const RESET = Date.UTC(2026, 9, 21);

function view(overrides: Partial<UsageView> = {}): UsageView {
  return {
    status: makeUsage({ policyConfigured: true, used: 12, resetAtMs: RESET }),
    error: null,
    loading: false,
    reload: vi.fn(),
    now: RESET - 3 * 3_600_000,
    ...overrides,
  };
}

describe("UsageMeter", () => {
  it("reports a configured local limit from the core record", () => {
    render(<UsageMeter usage={view()} />);

    expect(screen.getByText("38 of 50 left")).toBeTruthy();
    expect(screen.getByText(/Resets in 3h/)).toBeTruthy();
    expect(screen.getByText(/resets at 00:00 UTC/i)).toBeTruthy();
  });

  it("exposes configured local usage to assistive technology", () => {
    render(<UsageMeter usage={view()} />);

    const bar = screen.getByRole("progressbar", { name: "Local generations used" });

    expect(bar.getAttribute("aria-valuenow")).toBe("12");
    expect(bar.getAttribute("aria-valuemax")).toBe("50");
    expect(bar.getAttribute("aria-valuetext")).toBe("12 of 50 generations used");
  });

  it("states configured local exhaustion plainly and offers no upsell", () => {
    render(
      <UsageMeter
        usage={view({
          status: makeUsage({
            policyConfigured: true,
            used: 50,
            exhausted: true,
            remaining: 0,
          }),
          now: RESET - 60_000,
        })}
      />,
    );

    expect(screen.getByText("50 of 50 used")).toBeTruthy();
    expect(screen.getByText(/Configured local allowance reached/i)).toBeTruthy();
    expect(screen.queryByText(/upgrade|buy|purchase|subscribe/i)).toBeNull();
  });

  it("shows informational local usage without inventing a quota", () => {
    render(<UsageMeter usage={view({ status: makeUsage({ used: 64 }) })} />);

    expect(screen.getByText("64 recorded today")).toBeTruthy();
    expect(screen.getByText(/informational; no local daily limit is configured/i)).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.queryByText(/of 50 left/i)).toBeNull();
  });

  it("keeps local inference available when an unconfigured count is unknown", () => {
    render(<UsageMeter usage={view({ status: makeUsage({ used: null }) })} />);

    expect(screen.getByText("Count unknown")).toBeTruthy();
    expect(screen.getByText(/Local inference remains available/i)).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });

  it("reports an IPC failure instead of inventing local usage", () => {
    render(<UsageMeter usage={view({ status: null, error: "the core did not answer" })} />);

    expect(screen.getByRole("alert").textContent).toContain("Local usage is unavailable");
    expect(screen.queryByText(/recorded today/)).toBeNull();
  });

  it("surfaces a preserved record notice and a failed write", () => {
    render(
      <UsageMeter
        usage={view({
          status: makeUsage({
            used: null,
            notice: "The existing usage record was not trusted and was preserved.",
            error: "the usage record could not be written (permission denied)",
          }),
        })}
      />,
    );

    expect(screen.getByText(/existing usage record was not trusted/i)).toBeTruthy();
    expect(screen.getByText(/could not be updated/i)).toBeTruthy();
  });

  it("shows a loading state without a fabricated counter", () => {
    render(<UsageMeter usage={view({ loading: true, status: null })} />);

    expect(screen.getByText("Checking local usage…")).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });
});
