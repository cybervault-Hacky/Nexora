import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { checkBlenderInstallation, isNativeInstallationResult } from "./native";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
  Reflect.set(window, "__TAURI_INTERNALS__", {});
});

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

describe("Blender detection IPC boundary", () => {
  it("accepts the typed native variants and rejects UI-only or incomplete payloads", () => {
    expect(isNativeInstallationResult({ state: "unsupported", reason: "Not implemented." })).toBe(true);
    expect(isNativeInstallationResult({ state: "not_found", checkedLocations: [], rejectedCandidates: 0 })).toBe(true);
    expect(isNativeInstallationResult({
      state: "found",
      installations: [{
        executablePath: "C:\\Program Files\\Blender Foundation\\Blender 4.2\\blender.exe",
        version: null,
        method: "standard_directory",
        confidence: "candidate_only",
        validation: "Candidate only; identity not verified.",
      }],
      incompleteLocations: [],
      rejectedCandidates: 0,
    })).toBe(true);
    expect(isNativeInstallationResult({ state: "error", code: "io", message: "Failed", incompleteLocations: [] })).toBe(true);
    expect(isNativeInstallationResult({ state: "checking" })).toBe(false);
    expect(isNativeInstallationResult({ state: "unknown" })).toBe(false);
    expect(isNativeInstallationResult({ state: "found", installations: [] })).toBe(false);
    expect(isNativeInstallationResult({ state: "not_found", checkedLocations: [] })).toBe(false);
  });

  it("uses one explicit installation check and preserves the native result without inferring readiness", async () => {
    invokeMock.mockResolvedValue({
      state: "unsupported",
      reason: "Windows detection is not implemented or verified.",
    });

    const result = await checkBlenderInstallation();

    expect(result).toEqual({
      state: "unsupported",
      reason: "Windows detection is not implemented or verified.",
    });
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("get_blender_detection_status");
  });

  it("coalesces concurrent explicit checks into one native IPC call", async () => {
    invokeMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          setTimeout(() => resolve({ state: "unsupported", reason: "No detector." }), 0);
        }),
    );

    const first = checkBlenderInstallation();
    const second = checkBlenderInstallation();
    await expect(Promise.all([first, second])).resolves.toEqual([
      { state: "unsupported", reason: "No detector." },
      { state: "unsupported", reason: "No detector." },
    ]);
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it("reports malformed native payloads and IPC failures as typed detection errors", async () => {
    invokeMock.mockResolvedValueOnce({ state: "found", version: "untrusted" });
    await expect(checkBlenderInstallation()).resolves.toEqual({
      state: "error",
      code: "invalid_response",
      message: "The desktop core returned an invalid Blender detection status.",
      incompleteLocations: [],
    });

    invokeMock.mockRejectedValueOnce(new Error("private native detail"));
    await expect(checkBlenderInstallation()).resolves.toEqual({
      state: "error",
      code: "ipc",
      message: "The desktop core could not report Blender installation detection.",
      incompleteLocations: [],
    });
  });

  it("returns unsupported in a browser preview without invoking native commands", async () => {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");

    await expect(checkBlenderInstallation()).resolves.toEqual({
      state: "unsupported",
      reason: "The browser preview cannot perform Windows desktop installation checks; no files were scanned.",
    });
    expect(invokeMock).not.toHaveBeenCalled();
  });
});
