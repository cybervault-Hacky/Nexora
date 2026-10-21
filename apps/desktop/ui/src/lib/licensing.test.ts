import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
  licenceActivate,
  licenceDeactivate,
  licenceGetStatus,
  PLAN_TIERS,
} from "./licensing";
import { getIpcTelemetryEvents, clearIpcTelemetry } from "./telemetry";
import { PUBLISHED_DAILY_ALLOWANCE } from "./usage";
import { markDesktop } from "../test/fixtures";

describe("Phase 26 — Licensing Client & IPC Contracts", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    clearIpcTelemetry();
    markDesktop(true);
  });

  it("exports valid plan tiers with community tier as default", () => {
    expect(PLAN_TIERS.length).toBe(3);
    const community = PLAN_TIERS.find((p) => p.id === "community");
    expect(community).toBeDefined();
    expect(community?.price).toBe("$0");
  });

  it("never promises unlimited local inference", () => {
    for (const plan of PLAN_TIERS) {
      expect(plan.features.some((feature) => /unlimited/i.test(feature.name))).toBe(false);
      expect(
        plan.features.some((feature) =>
          feature.name.startsWith(`${PUBLISHED_DAILY_ALLOWANCE} local generations per day`),
        ),
      ).toBe(true);
    }
  });

  it("licenceGetStatus invokes licence_get_status and records telemetry", async () => {
    invokeMock.mockResolvedValueOnce({
      tier: "pro",
      planName: "Developer Pro Edition",
      active: true,
      licenceKey: "NEXORA-PRO-PERPETUAL_DEV-1234",
      licensee: "DEV",
      expiresAt: null,
      features: ["commercial_use_rights"],
    });

    const status = await licenceGetStatus();
    expect(invokeMock).toHaveBeenCalledWith("licence_get_status");
    expect(status.tier).toBe("pro");
    expect(status.licensee).toBe("DEV");

    const events = getIpcTelemetryEvents();
    expect(events.some((e) => e.command === "licence_get_status")).toBe(true);
  });

  it("licenceActivate invokes licence_activate with payload", async () => {
    invokeMock.mockResolvedValueOnce({
      tier: "team",
      planName: "Team Commercial Edition",
      active: true,
      licenceKey: "NEXORA-TEAM-PERPETUAL_ACME-5678",
      licensee: "ACME",
      expiresAt: null,
      features: ["multi_seat_workspaces"],
    });

    const status = await licenceActivate({
      licenceKey: "NEXORA-TEAM-PERPETUAL_ACME-5678",
    });

    expect(invokeMock).toHaveBeenCalledWith("licence_activate", {
      request: { licenceKey: "NEXORA-TEAM-PERPETUAL_ACME-5678" },
    });
    expect(status.tier).toBe("team");
    expect(status.licensee).toBe("ACME");
  });

  it("licenceDeactivate invokes licence_deactivate", async () => {
    invokeMock.mockResolvedValueOnce({
      tier: "community",
      planName: "Community Edition",
      active: true,
      licenceKey: null,
      licensee: null,
      expiresAt: null,
      features: [],
    });

    const status = await licenceDeactivate();
    expect(invokeMock).toHaveBeenCalledWith("licence_deactivate");
    expect(status.tier).toBe("community");
  });

  it("falls back to default community tier when outside desktop shell", async () => {
    markDesktop(false);

    const status = await licenceGetStatus();
    expect(status.tier).toBe("community");
    expect(status.active).toBe(true);

    await expect(
      licenceActivate({ licenceKey: "NEXORA-PRO-TEST-1234" }),
    ).rejects.toMatchObject({
      code: "desktop_required",
    });
  });
});
