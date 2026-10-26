import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { checkBlenderInstallation } from "../features/integrations/native";
import type { InstallationStatus } from "../features/integrations/catalogue";
import { AppIntegrationsPage } from "./AppIntegrationsPage";

vi.mock("../features/integrations/native", () => ({
  checkBlenderInstallation: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(checkBlenderInstallation).mockClear();
  vi.mocked(checkBlenderInstallation).mockResolvedValue({
    state: "unsupported",
    reason: "The Blender detector is not implemented or verified in this build.",
  });
});

describe("AppIntegrationsPage", () => {
  it("shows the shared catalogue heading, search, filters, and candidates", () => {
    render(<AppIntegrationsPage />);

    expect(screen.getByRole("heading", { name: "App Integrations" })).toBeTruthy();
    expect(screen.getByRole("searchbox", { name: "Search apps or capabilities" })).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "Filter by category" })).toBeTruthy();
    expect(screen.getByRole("list", { name: "Application catalogue" }).children).toHaveLength(3);
    expect(screen.getByText(/Catalogue only/)).toBeTruthy();
  });

  it("searches by app name and capability and filters by category", async () => {
    const user = userEvent.setup();
    render(<AppIntegrationsPage />);
    const search = screen.getByRole("searchbox", { name: "Search apps or capabilities" });

    await user.type(search, "mesh");
    expect(screen.getByRole("heading", { name: "Blender" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Figma" })).toBeNull();

    await user.clear(search);
    await user.type(search, "vscode");
    expect(screen.getByRole("heading", { name: "Visual Studio Code" })).toBeTruthy();

    await user.clear(search);
    await user.selectOptions(screen.getByRole("combobox", { name: "Filter by category" }), "design");
    expect(screen.getByRole("heading", { name: "Figma" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Blender" })).toBeNull();
  });

  it("shows a useful empty state with a working clear-filters action", async () => {
    const user = userEvent.setup();
    render(<AppIntegrationsPage />);
    await user.type(screen.getByRole("searchbox", { name: "Search apps or capabilities" }), "nothing matches");

    expect(screen.getByRole("status", { name: "No matching applications" })).toBeTruthy();
    expect(screen.queryByRole("list", { name: "Application catalogue" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    expect(screen.getByRole("list", { name: "Application catalogue" }).children).toHaveLength(3);
  });

  it("does not scan on render and announces an explicit check accessibly", async () => {
    const user = userEvent.setup();
    let resolveDetection!: (status: InstallationStatus) => void;
    vi.mocked(checkBlenderInstallation).mockReturnValueOnce(
      new Promise((resolve) => {
        resolveDetection = resolve;
      }),
    );
    render(<AppIntegrationsPage />);

    expect(within(screen.getByRole("article", { name: "Blender" })).getByText("Unknown — installation has not been checked")).toBeTruthy();
    expect(checkBlenderInstallation).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Check Installation" }));
    const loadingStatus = screen.getByText("Checking supported installation locations…");
    expect(loadingStatus.closest("dd")?.getAttribute("aria-live")).toBe("polite");
    expect(screen.getByRole("button", { name: "Checking…" }).hasAttribute("disabled")).toBe(true);
    resolveDetection({ state: "unsupported", reason: "No detector is available in this build." });
    expect(await screen.findByText(/Detection unsupported — No detector is available/)).toBeTruthy();
  });

  it("reports unsupported detection without implying connected or ready", async () => {
    const user = userEvent.setup();
    render(<AppIntegrationsPage />);
    const blender = screen.getByRole("article", { name: "Blender" });
    const card = within(blender);

    expect(card.getByText("Planned", { selector: ".integration-status" })).toBeTruthy();
    expect(checkBlenderInstallation).not.toHaveBeenCalled();
    await user.click(card.getByRole("button", { name: "Check Installation" }));
    expect(await card.findByText(/Detection unsupported —/)).toBeTruthy();
    expect(card.getByText(/Unavailable — no connector or validated connection handshake/)).toBeTruthy();
    expect(card.getByText(/Not ready — No validated Blender connector/)).toBeTruthy();
    expect(card.getByText(/Connection controls are unavailable/)).toBeTruthy();
    expect(card.queryByRole("button", { name: /connect|disconnect|reconnect|configure|detect/i })).toBeNull();
    expect(card.getByRole("button", { name: "Check Installation" })).toBeTruthy();
  });

  it("labels detections as candidate-only and does not infer connection or readiness", async () => {
    const user = userEvent.setup();
    vi.mocked(checkBlenderInstallation).mockResolvedValueOnce({
      state: "found",
      installations: [{
        executablePath: "C:\\Program Files\\Blender Foundation\\Blender 4.2\\blender.exe",
        version: null,
        method: "standard_directory",
        confidence: "candidate_only",
        validation: "Candidate only; publisher identity and Blender version are not verified.",
      }],
      incompleteLocations: [],
      rejectedCandidates: 1,
    });
    render(<AppIntegrationsPage />);

    const blender = within(screen.getByRole("article", { name: "Blender" }));
    await user.click(blender.getByRole("button", { name: "Check Installation" }));

    expect(await blender.findByText(/Found candidate only — C:\\Program Files/)).toBeTruthy();
    expect(blender.getByText(/Not ready — No validated Blender connector/)).toBeTruthy();
    expect(blender.getByText(/Unavailable — no connector or validated connection handshake/)).toBeTruthy();
    expect(blender.getByText(/Installation detection reports candidates only/)).toBeTruthy();
    expect(blender.queryByRole("button", { name: /connect|disconnect|reconnect/i })).toBeNull();
  });

  it("keeps setup and troubleshooting guidance in one shared location", () => {
    render(<AppIntegrationsPage />);
    expect(screen.getByRole("complementary", { name: "Integration help" })).toBeTruthy();
    expect(screen.getByText(/Setup guidance and troubleshooting will appear here/)).toBeTruthy();
  });
});
