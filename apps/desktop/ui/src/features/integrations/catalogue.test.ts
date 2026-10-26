import { describe, expect, it } from "vitest";
import {
  APP_INTEGRATIONS,
  CONNECTION_STATUS_LABELS,
  findDuplicateIntegrationIds,
  filterAppIntegrations,
} from "./catalogue";

describe("App Integrations catalogue", () => {
  it("provides complete, typed entries with stable unique identifiers", () => {
    const ids = APP_INTEGRATIONS.map((entry) => entry.id);
    expect(findDuplicateIntegrationIds(APP_INTEGRATIONS)).toEqual([]);
    expect(new Set(ids).size).toBe(ids.length);

    for (const entry of APP_INTEGRATIONS) {
      expect(entry.id.trim()).not.toBe("");
      expect(entry.name.trim()).not.toBe("");
      expect(entry.description.trim()).not.toBe("");
      expect(entry.category).toMatch(/^(3d|development|design)$/);
      expect(entry.capabilities.length).toBeGreaterThan(0);
      expect(entry.compatibility.verified).toBe(false);
      expect(entry.compatibility.versionRange).toBeNull();
    }
  });

  it("finds repeated IDs for registry validation", () => {
    expect(
      findDuplicateIntegrationIds([
        { id: "blender" },
        { id: "figma" },
        { id: "blender" },
        { id: "blender" },
      ]),
    ).toEqual(["blender"]);
  });

  it("searches application names, keywords, and capability names/descriptions", () => {
    expect(filterAppIntegrations(APP_INTEGRATIONS, { query: "figma" }).map((app) => app.id)).toEqual([
      "figma",
    ]);
    expect(filterAppIntegrations(APP_INTEGRATIONS, { query: "mesh" }).map((app) => app.id)).toEqual([
      "blender",
    ]);
    expect(filterAppIntegrations(APP_INTEGRATIONS, { query: "selected editor content" }).map((app) => app.id)).toEqual([
      "vscode",
    ]);
  });

  it("combines category and text filters and returns no matches honestly", () => {
    expect(filterAppIntegrations(APP_INTEGRATIONS, { category: "design" }).map((app) => app.id)).toEqual([
      "figma",
    ]);
    expect(
      filterAppIntegrations(APP_INTEGRATIONS, { query: "mesh", category: "design" }),
    ).toEqual([]);
    expect(filterAppIntegrations(APP_INTEGRATIONS, { query: "no such app" })).toEqual([]);
  });

  it("keeps planned candidates undetected and without connection actions", () => {
    expect(APP_INTEGRATIONS).toHaveLength(3);
    for (const app of APP_INTEGRATIONS) {
      expect(app.implementationStatus).toBe("planned");
      expect(app.installationStatus.state).toBe("unknown");
      expect(app.connectionStatus).toBeNull();
      expect(app.connectionActions).toEqual([]);
      expect(app.capabilities.every((capability) => capability.status === "planned")).toBe(true);
    }
    expect(APP_INTEGRATIONS.find((app) => app.id === "blender")?.taskReadiness).toEqual({
      state: "blocked",
      reasons: ["No validated Blender connector, handshake, or task executor is available."],
    });
  });

  it("provides user-facing labels for the shared connection lifecycle", () => {
    expect(CONNECTION_STATUS_LABELS.not_connected).toBe("Not connected");
    expect(CONNECTION_STATUS_LABELS.connecting).toBe("Connecting");
    expect(CONNECTION_STATUS_LABELS.connected).toBe("Connected");
    expect(CONNECTION_STATUS_LABELS.needs_setup).toBe("Needs setup");
    expect(CONNECTION_STATUS_LABELS.unsupported).toBe("Unsupported");
    expect(CONNECTION_STATUS_LABELS.error).toBe("Error");
    expect(CONNECTION_STATUS_LABELS.disconnected).toBe("Disconnected");
  });
});
