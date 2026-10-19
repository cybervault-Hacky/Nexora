import { beforeEach, describe, expect, it } from "vitest";
import {
  clearIpcTelemetry,
  getIpcTelemetryEvents,
  recordIpcTelemetry,
  subscribeIpcTelemetry,
} from "./telemetry";

describe("Phase 25 — In-memory IPC Telemetry", () => {
  beforeEach(() => {
    clearIpcTelemetry();
  });

  it("records telemetry events with privacy-safe duration and status", () => {
    recordIpcTelemetry("auth_get_session", 12.5, "ok");
    const events = getIpcTelemetryEvents();
    expect(events.length).toBe(1);
    expect(events[0]!.command).toBe("auth_get_session");
    expect(events[0]!.durationMs).toBe(13);
    expect(events[0]!.status).toBe("ok");
    expect(events[0]!.errorSummary).toBeUndefined();
  });

  it("records error summaries without exposing passwords or tokens", () => {
    recordIpcTelemetry("auth_change_password", 45, "error", "Invalid credentials");
    const events = getIpcTelemetryEvents();
    expect(events.length).toBe(1);
    expect(events[0]!.status).toBe("error");
    expect(events[0]!.errorSummary).toBe("Invalid credentials");
  });

  it("notifies subscribers when events are recorded", () => {
    let callCount = 0;
    const unsub = subscribeIpcTelemetry(() => {
      callCount += 1;
    });

    recordIpcTelemetry("load_model", 80, "ok");
    expect(callCount).toBe(1);

    unsub();
    recordIpcTelemetry("unload_model", 10, "ok");
    expect(callCount).toBe(1); // No further calls after unsubscribe
  });

  it("caps max recorded events at 100 to prevent memory growth", () => {
    for (let i = 0; i < 110; i++) {
      recordIpcTelemetry(`cmd_${i}`, 5, "ok");
    }
    const events = getIpcTelemetryEvents();
    expect(events.length).toBe(100);
    // Most recent should be at the front
    expect(events[0]!.command).toBe("cmd_109");
  });
});
