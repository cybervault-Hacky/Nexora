import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ModelReadinessCard } from "./ModelReadinessCard";
import { READINESS_STATES } from "../lib/ipc";
import { makeReadiness } from "../test/fixtures";

describe("ModelReadinessCard", () => {
  it("labels every state the core can emit", () => {
    for (const state of READINESS_STATES) {
      const readiness = makeReadiness({ state });
      const { unmount } = render(
        <ModelReadinessCard readiness={{ ...readiness, headline: `headline-${state}` }} />,
      );

      expect(screen.getByText(`headline-${state}`)).toBeTruthy();
      expect(screen.getByTestId("model-readiness").getAttribute("data-state")).toBe(state);
      expect(document.querySelector(`.readiness__badge[data-state="${state}"]`)).toBeTruthy();

      unmount();
    }
  });

  it("shows the backend's explanation, not a generic one", () => {
    render(
      <ModelReadinessCard
        readiness={makeReadiness({
          state: "insufficient_memory",
          headline: "Not enough memory",
          detail:
            "The model is installed and verified, but this machine does not have enough available memory to load it. About 10.9 GiB is needed. An estimate, not a hard minimum.",
          canDownload: false,
          canLoad: false,
        })}
      />,
    );

    expect(screen.getByRole("heading", { name: "Not enough memory" })).toBeTruthy();
    expect(
      screen.getByText(/does not have enough available memory to load it/i),
    ).toBeTruthy();
    expect(screen.getByText(/An estimate, not a hard minimum/i)).toBeTruthy();
  });

  it("renders each check with its status word, not only a colour", () => {
    render(
      <ModelReadinessCard
        readiness={makeReadiness({
          state: "insufficient_storage",
          headline: "Not enough storage",
          checks: [
            { id: "storage", status: "blocked", message: "Only 4.00 GiB free." },
            { id: "memory", status: "warning", message: "Memory may change." },
            { id: "integrity", status: "unknown", message: "Not checked yet." },
            { id: "partial", status: "ok", message: "No partial file." },
          ],
        })}
      />,
    );

    expect(screen.getByText("Blocked")).toBeTruthy();
    expect(screen.getByText("Warning")).toBeTruthy();
    expect(screen.getByText("Unknown")).toBeTruthy();
    expect(screen.getByText("OK")).toBeTruthy();
    expect(document.querySelector('[data-check="storage"][data-status="blocked"]')).toBeTruthy();
  });

  it("shows the model file size and the free space needed separately", () => {
    render(<ModelReadinessCard readiness={makeReadiness()} />);

    const fileFact = screen.getByText("Model file").closest("div") as HTMLElement;
    const spaceFact = screen.getByText("Free space needed").closest("div") as HTMLElement;
    const memoryFact = screen.getByText("Memory to load").closest("div") as HTMLElement;

    expect(within(fileFact).getByText("8.37 GiB")).toBeTruthy();
    expect(within(spaceFact).getByText(/8\.87 GiB/)).toBeTruthy();
    expect(within(spaceFact).getByText(/file plus reserve/i)).toBeTruthy();
    expect(within(memoryFact).getByText(/≈10\.9 GiB/)).toBeTruthy();
  });

  it("always marks the numbers as estimates", () => {
    render(<ModelReadinessCard readiness={makeReadiness()} />);

    expect(
      screen.getByText(/are estimates from the published file size/i),
    ).toBeTruthy();
  });

  it("offers a refresh without inventing a state", async () => {
    const onRefresh = vi.fn();
    const user = userEvent.setup();

    render(<ModelReadinessCard readiness={makeReadiness()} onRefresh={onRefresh} />);

    await user.click(screen.getByRole("button", { name: /Check again/i }));

    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("says it is still reading instead of guessing", () => {
    render(<ModelReadinessCard readiness={null} />);

    expect(screen.getByText(/Reading the model's readiness/i)).toBeTruthy();
  });

  it("reports a read failure instead of showing the last known state", () => {
    render(<ModelReadinessCard readiness={null} error="The desktop core did not answer." />);

    expect(screen.getByRole("alert").textContent).toMatch(/did not answer/);
    expect(screen.queryByText("Ready to download")).toBeNull();
  });
});
