import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mockGetDiagnostics = vi.fn();

vi.mock("../lib/auth", () => ({
  getDeveloperDiagnostics: () => mockGetDiagnostics(),
}));

import { clearIpcTelemetry, recordIpcTelemetry } from "../lib/telemetry";
import { FounderPage } from "./FounderPage";
import { makeStatus } from "../test/fixtures";

describe("Phase 25 — FounderPage", () => {
  beforeEach(() => {
    clearIpcTelemetry();
    mockGetDiagnostics.mockReset();
    mockGetDiagnostics.mockResolvedValue({
      platform: "linux",
      arch: "x86_64",
      appVersion: "0.1.0",
      credentialVault: "Keyring OS Vault",
      authServiceUrl: "http://127.0.0.1:8787",
      cspEnforced: true,
      offlineCapable: true,
    });
  });

  it("renders system diagnostics and engine state", async () => {
    const status = makeStatus({
      modelPath: "/models/qwen2.5-coder-7b.gguf",
      contextSize: 32768,
      state: "ready",
    });

    render(<FounderPage status={status} />);

    expect(screen.getByRole("heading", { name: "Founder Control Centre" })).toBeTruthy();
    expect(await screen.findByText("linux (x86_64)")).toBeTruthy();
    expect(screen.getByText("Keyring OS Vault")).toBeTruthy();
    expect(screen.getByText("32768 tokens max window")).toBeTruthy();
  });

  it("displays real-time IPC telemetry traffic and allows filtering", async () => {
    recordIpcTelemetry("load_model", 120, "ok");
    recordIpcTelemetry("auth_login", 45, "error", "Invalid credentials");

    render(<FounderPage status={null} />);

    expect(screen.getByText("load_model")).toBeTruthy();
    expect(screen.getByText("auth_login")).toBeTruthy();
    expect(screen.getByText("(Invalid credentials)")).toBeTruthy();

    const user = userEvent.setup();
    const select = screen.getByLabelText("Filter events");

    // Filter to Errors only
    await user.selectOptions(select, "error");
    expect(screen.queryByText("load_model")).toBeNull();
    expect(screen.getByText("auth_login")).toBeTruthy();

    // Clear telemetry
    const clearBtn = screen.getByRole("button", { name: "Clear" });
    await user.click(clearBtn);
    expect(screen.getByText(/No IPC telemetry events recorded yet/i)).toBeTruthy();
  });

  it("probes backend auth connectivity on button click", async () => {
    const user = userEvent.setup();
    render(<FounderPage status={null} />);

    const pingBtn = screen.getByRole("button", { name: /Ping probe/i });
    await user.click(pingBtn);

    await waitFor(() => {
      expect(mockGetDiagnostics).toHaveBeenCalled();
    });

    expect(await screen.findByText(/Auth service configured at http:\/\/127\.0\.0\.1:8787/i)).toBeTruthy();
  });
});
