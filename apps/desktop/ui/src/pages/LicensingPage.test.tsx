import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mockUseLicence = vi.fn();

vi.mock("../features/licensing/useLicence", () => ({
  useLicence: () => mockUseLicence(),
}));

import { LicensingPage } from "./LicensingPage";

describe("Phase 26 — LicensingPage", () => {
  beforeEach(() => {
    mockUseLicence.mockReset();
  });

  it("renders local licence status and activation without a commercial plan comparison", () => {
    mockUseLicence.mockReturnValue({
      licence: {
        tier: "community",
        planName: "Community Edition",
        active: true,
        licenceKey: null,
        licensee: null,
        expiresAt: null,
        features: [],
      },
      loading: false,
      error: null,
      activateKey: vi.fn(),
      deactivateKey: vi.fn(),
    });

    render(<LicensingPage />);

    expect(screen.getByRole("heading", { name: "Developer Licensing" })).toBeTruthy();
    expect(screen.getByText("Community Edition")).toBeTruthy();
    expect(screen.getByText("Offline licence status")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Activate Offline Licence" })).toBeTruthy();
    expect(screen.queryByText(/Developer Pro|Team & Enterprise|\$19|\$49|perpetual/i)).toBeNull();
  });

  it("renders active Pro licence details with licensee and validity", () => {
    mockUseLicence.mockReturnValue({
      licence: {
        tier: "pro",
        planName: "Developer Pro Edition",
        active: true,
        licenceKey: "NEXORA-PRO-ACME-1234",
        licensee: "ACME Corp",
        expiresAt: null,
        features: ["commercial_use_rights"],
      },
      loading: false,
      error: null,
      activateKey: vi.fn(),
      deactivateKey: vi.fn(),
    });

    render(<LicensingPage />);

    expect(screen.getByText("Developer Pro Edition")).toBeTruthy();
    expect(screen.getByText("ACME Corp")).toBeTruthy();
    expect(screen.getByText("No expiry date recorded")).toBeTruthy();
    expect(screen.queryByText(/perpetual/i)).toBeNull();
    expect(screen.getByRole("button", { name: /Deactivate licence/i })).toBeTruthy();
  });

  it("submits valid licence key and displays success notification", async () => {
    const user = userEvent.setup();
    const activateKeyMock = vi.fn().mockResolvedValue({
      tier: "pro",
      planName: "Developer Pro Edition",
      active: true,
      licenceKey: "NEXORA-PRO-DEV-9999",
      licensee: "DEV",
      expiresAt: null,
      features: [],
    });

    mockUseLicence.mockReturnValue({
      licence: {
        tier: "community",
        planName: "Community Edition",
        active: true,
        licenceKey: null,
        licensee: null,
        expiresAt: null,
        features: [],
      },
      loading: false,
      error: null,
      activateKey: activateKeyMock,
      deactivateKey: vi.fn(),
    });

    render(<LicensingPage />);

    const input = screen.getByRole("textbox", { name: "Licence Key" });
    const submitBtn = screen.getByRole("button", { name: /Activate key/i });

    await user.type(input, "NEXORA-PRO-DEV-9999");
    await user.click(submitBtn);

    await waitFor(() => {
      expect(activateKeyMock).toHaveBeenCalledWith("NEXORA-PRO-DEV-9999");
    });

    expect(await screen.findByText("Successfully activated Developer Pro Edition!")).toBeTruthy();
  });

  it("deactivates licence on button click", async () => {
    const user = userEvent.setup();
    const deactivateKeyMock = vi.fn().mockResolvedValue({
      tier: "community",
      planName: "Community Edition",
      active: true,
      licenceKey: null,
      licensee: null,
      expiresAt: null,
      features: [],
    });

    mockUseLicence.mockReturnValue({
      licence: {
        tier: "pro",
        planName: "Developer Pro Edition",
        active: true,
        licenceKey: "NEXORA-PRO-DEV-9999",
        licensee: "DEV",
        expiresAt: null,
        features: [],
      },
      loading: false,
      error: null,
      activateKey: vi.fn(),
      deactivateKey: deactivateKeyMock,
    });

    render(<LicensingPage />);

    const deactivateBtn = screen.getByRole("button", { name: /Deactivate licence/i });
    await user.click(deactivateBtn);

    await waitFor(() => {
      expect(deactivateKeyMock).toHaveBeenCalled();
    });

    expect(await screen.findByText("Licence deactivated. Reverted to Community Edition.")).toBeTruthy();
  });
});
