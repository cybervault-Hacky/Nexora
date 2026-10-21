import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { UsageMeter } from "./UsageMeter";
import { makeUsage } from "../test/fixtures";
import type { UsageView } from "../features/usage/useUsage";

const RESET = Date.UTC(2026, 9, 21);

function view(overrides: Partial<UsageView> = {}): UsageView {
  return {
    status: makeUsage({ used: 12, resetAtMs: RESET }),
    error: null,
    loading: false,
    reload: vi.fn(),
    now: RESET - 3 * 3_600_000,
    ...overrides,
  };
}

describe("Phase 7 — UsageMeter", () => {
  it("reports remaining usage from the authoritative record", () => {
    render(<UsageMeter usage={view()} />);

    expect(screen.getByText("38 of 50 left")).toBeTruthy();
    expect(screen.getByText(/Resets in 3h/)).toBeTruthy();
    expect(screen.getByText(/resets at 00:00 UTC/i)).toBeTruthy();
  });

  it("exposes the used allowance to assistive technology", () => {
    render(<UsageMeter usage={view()} />);

    const bar = screen.getByRole("progressbar", { name: "Daily generations used" });

    expect(bar.getAttribute("aria-valuenow")).toBe("12");
    expect(bar.getAttribute("aria-valuemax")).toBe("50");
    expect(bar.getAttribute("aria-valuetext")).toBe("12 of 50 generations used");
  });

  it("states exhaustion plainly and offers no upsell", () => {
    render(
      <UsageMeter
        usage={view({
          status: makeUsage({ used: 50, exhausted: true, remaining: 0 }),
          now: RESET - 60_000,
        })}
      />,
    );

    expect(screen.getByText("50 of 50 used")).toBeTruthy();
    expect(screen.getByText(/Daily free allowance reached/i)).toBeTruthy();
    expect(screen.queryByText(/upgrade|buy|purchase|subscribe/i)).toBeNull();
  });

  it("shows a provisioning note without pretending usage exists", () => {
    render(<UsageMeter usage={view({ loading: true, status: null })} />);

    expect(screen.getByText("Checking daily usage…")).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });

  it("reports an unreadable record as an error, not as full allowance", () => {
    render(<UsageMeter usage={view({ status: null, error: "the core did not answer" })} />);

    expect(screen.getByRole("alert").textContent).toContain("Daily usage is unavailable");
    expect(screen.queryByText(/of 50 left/)).toBeNull();
  });

  it("surfaces a repaired record and a failed write", () => {
    render(
      <UsageMeter
        usage={view({
          status: makeUsage({
            used: 1,
            notice: "The stored usage record was unusable, so today's allowance was restarted.",
            error: "the usage record could not be written (permission denied)",
          }),
        })}
      />,
    );

    expect(screen.getByText(/today's allowance was restarted/i)).toBeTruthy();
    expect(screen.getByText(/could not be updated/i)).toBeTruthy();
  });

  it("says the allowance is provisional rather than implying it is final", () => {
    render(<UsageMeter usage={view()} />);

    expect(screen.getByText(/allowance is provisional/i)).toBeTruthy();
  });
});
