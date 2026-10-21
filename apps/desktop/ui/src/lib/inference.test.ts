import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/**
 * Runtime tests for the IPC wrapper.
 *
 * The contract test proves the *source* names agree; this file proves the
 * wrapper's actual call shape and its error normalisation at runtime, with the
 * Tauri module replaced by a double. It is a frontend test: no Rust, no Tauri,
 * no native window is involved.
 */

// `vi.mock` is hoisted above imports, so the doubles must be hoisted too.
const { invoke, Channel } = vi.hoisted(() => {
  const spy = vi.fn();

  class TestChannel<T> {
    onmessage: ((event: T) => void) | null = null;
  }

  return { invoke: spy, Channel: TestChannel };
});

vi.mock("@tauri-apps/api/core", () => ({ invoke, Channel }));

import {
  cancelGeneration,
  cancelModelDownload,
  fetchModelProfile,
  fetchStatus,
  generate,
  startModelDownload,
  toInferenceError,
  validateModel,
  verifyModel,
} from "./inference";
import { isDesktopShell, isEngineBusy, isEngineReady, previewStatus } from "./ipc";

function enterDesktopShell() {
  Reflect.set(window, "__TAURI_INTERNALS__", {});
}

beforeEach(() => {
  invoke.mockReset();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

describe("error normalisation", () => {
  it("keeps a typed core error intact", () => {
    const failure = { code: "busy", message: "A response is already being generated." };

    expect(toInferenceError(failure)).toEqual(failure);
  });

  it("wraps a rejection that is only a string", () => {
    expect(toInferenceError("command not found")).toEqual({
      code: "ipc",
      message: "command not found",
    });
  });

  it("wraps an Error", () => {
    expect(toInferenceError(new Error("channel closed"))).toEqual({
      code: "ipc",
      message: "channel closed",
    });
  });

  it("never returns an empty message", () => {
    for (const cause of [null, undefined, 42, {}, { code: 1, message: "x" }, ""]) {
      const failure = toInferenceError(cause);

      expect(failure.code).toBe("ipc");
      expect(failure.message.trim().length).toBeGreaterThan(0);
    }
  });

  it("does not trust half a payload", () => {
    // A `code` without a `message` would render as "undefined" in a banner.
    expect(toInferenceError({ code: "busy" })).toEqual({
      code: "ipc",
      message: "The desktop core did not answer.",
    });
  });
});

describe("engine status helpers", () => {
  const status = (state: string) => ({
    name: "Nexora",
    version: "0.1.0",
    engine: { state, runtime: null, model: null, detail: "" },
    modelPath: null,
    runtimePath: null,
    contextSize: 32768,
  });

  it("treats only ready and generating as usable", () => {
    expect(isEngineReady(status("ready") as never)).toBe(true);
    expect(isEngineReady(status("generating") as never)).toBe(true);

    for (const state of ["not_configured", "stopped", "loading", "error"]) {
      expect(isEngineReady(status(state) as never)).toBe(false);
    }
  });

  it("refuses to call an unknown state ready", () => {
    // A future or malformed state must fail closed, not open.
    expect(isEngineReady(status("surprise") as never)).toBe(false);
    expect(isEngineReady(null)).toBe(false);
  });

  it("only reports busy while generating", () => {
    expect(isEngineBusy(status("generating") as never)).toBe(true);
    expect(isEngineBusy(status("ready") as never)).toBe(false);
    expect(isEngineBusy(null)).toBe(false);
  });

  it("never says the browser preview is ready", () => {
    expect(previewStatus().engine.state).toBe("not_configured");
    expect(isEngineReady(previewStatus())).toBe(false);
  });
});

describe("outside the desktop shell", () => {
  it("is detected as a plain browser", () => {
    expect(isDesktopShell()).toBe(false);
  });

  it("reports the preview status instead of calling IPC", async () => {
    const status = await fetchStatus();

    expect(status.engine.state).toBe("not_configured");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("refuses to generate and explains why", async () => {
    await expect(generate({ requestId: "a", messages: [] }, () => {})).rejects.toEqual({
      code: "desktop_only",
      message: expect.stringContaining("desktop app"),
    });

    expect(invoke).not.toHaveBeenCalled();
  });

  it("reports that nothing was cancelled", async () => {
    await expect(cancelGeneration("a")).resolves.toBe(false);
    expect(invoke).not.toHaveBeenCalled();
  });

  it("cannot start a model download from a browser preview", async () => {
    await expect(startModelDownload(null, "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m", () => {})).rejects.toEqual({
      code: "desktop_only",
      message: expect.stringContaining("desktop app"),
    });

    await expect(cancelModelDownload()).resolves.toBe(false);
    await expect(verifyModel("C:/models/a.gguf")).rejects.toEqual({
      code: "desktop_only",
      message: expect.stringContaining("desktop app"),
    });

    expect(invoke).not.toHaveBeenCalled();
  });

  it("cannot read model metadata from a browser preview", async () => {
    await expect(fetchModelProfile()).rejects.toEqual({
      code: "desktop_only",
      message: expect.stringContaining("desktop app"),
    });

    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("inside the desktop shell", () => {
  beforeEach(enterDesktopShell);

  it("reads status through the registered command", async () => {
    const payload = {
      name: "Nexora",
      version: "0.1.0",
      engine: { state: "ready", runtime: "b11714", model: "qwen", detail: "" },
      modelPath: "C:/models/qwen.gguf",
      runtimePath: "C:/tools/llama-server.exe",
      contextSize: 16384,
    };

    invoke.mockResolvedValue(payload);

    await expect(fetchStatus()).resolves.toEqual(payload);
    expect(invoke).toHaveBeenCalledWith("get_app_status", undefined);
  });

  it("sends the streaming request and channel under the names Rust expects", async () => {
    invoke.mockResolvedValue({ requestId: "req-1", finishReason: "stop", cancelled: false });

    const request = {
      requestId: "req-1",
      messages: [{ role: "user" as const, content: "hello" }],
      maxTokens: 1024,
    };

    await generate(request, () => {});

    expect(invoke).toHaveBeenCalledTimes(1);

    const [command, args] = invoke.mock.calls[0] as [string, Record<string, unknown>];

    expect(command).toBe("start_generation");
    expect(Object.keys(args).sort()).toEqual(["onEvent", "request"]);
    expect(args.request).toEqual(request);
    expect(args.onEvent).toBeInstanceOf(Channel);
  });

  it("delivers channel messages to the caller unchanged", async () => {
    const received: unknown[] = [];

    invoke.mockImplementation(
      async (_command: string, args: { onEvent: { onmessage: ((event: unknown) => void) | null } }) => {
        args.onEvent.onmessage?.({ type: "started", requestId: "req-1" });
        args.onEvent.onmessage?.({ type: "delta", requestId: "req-1", text: "hi" });
        // Malformed and unknown events are passed through; the consumer filters.
        args.onEvent.onmessage?.({ type: "delta", requestId: "req-1" });
        args.onEvent.onmessage?.({ type: "finished" });

        return { requestId: "req-1", finishReason: "stop", cancelled: false };
      },
    );

    await generate({ requestId: "req-1", messages: [] }, (event) => received.push(event));

    expect(received).toEqual([
      { type: "started", requestId: "req-1" },
      { type: "delta", requestId: "req-1", text: "hi" },
      { type: "delta", requestId: "req-1" },
      { type: "finished" },
    ]);
  });

  it("preserves a typed core rejection", async () => {
    invoke.mockRejectedValue({ code: "not_ready", message: "The engine is not responding." });

    await expect(generate({ requestId: "a", messages: [] }, () => {})).rejects.toEqual({
      code: "not_ready",
      message: "The engine is not responding.",
    });
  });

  it("normalises a transport-level rejection", async () => {
    invoke.mockRejectedValue("unknown command start_generation");

    await expect(generate({ requestId: "a", messages: [] }, () => {})).rejects.toEqual({
      code: "ipc",
      message: "unknown command start_generation",
    });
  });

  it("sends the live request id when cancelling", async () => {
    invoke.mockResolvedValue(true);

    await expect(cancelGeneration("req-9")).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith("cancel_generation", { requestId: "req-9" });
  });

  it("reports a failed cancel as false rather than throwing", async () => {
    invoke.mockRejectedValue(new Error("gone"));

    await expect(cancelGeneration("req-9")).resolves.toBe(false);
  });

  it("reads the approved profile without sending an identifier", async () => {
    const profile = {
      id: "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m",
      displayName: "Nexora Mini",
      publisher: "Qwen team, Alibaba Group",
      upstreamModel: "Qwen/Qwen2.5-Coder-7B-Instruct",
      upstreamRevision: "c03e6d358207e414f1eca0bb1891e29f1db0e242",
      format: "GGUF",
      quantization: "Q4_K_M",
      licence: { id: "Apache-2.0", reference: "https://example.invalid/LICENSE" },
      source: {
        repository: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF",
        revision: "13fb94bfda8c8cf22497dc57b78f391a9acb426a",
        fileName: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
        sizeBytes: 4_683_073_536,
        sha256: "509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c",
      },
      context: { recommended: 32_768, documentedMax: 32_768 },
      compatibilityNotes: ["note"],
      locallyVerified: false,
    };

    invoke.mockResolvedValue(profile);

    await expect(fetchModelProfile()).resolves.toEqual(profile);
    expect(invoke).toHaveBeenCalledWith("get_model_profile", undefined);
  });

  it("normalises a rejected profile lookup", async () => {
    invoke.mockRejectedValue({ code: "unknown_model", message: "No model profile is registered." });

    await expect(fetchModelProfile()).rejects.toEqual({
      code: "unknown_model",
      message: "No model profile is registered.",
    });
  });

  it("starts a download with the folder and a progress channel", async () => {
    const outcome = {
      path: "C:/models/m.gguf",
      fileName: "m.gguf",
      sizeBytes: 10,
      sha256: "abc",
      cancelled: false,
    };

    invoke.mockImplementation(
      async (
        _command: string,
        args: { onEvent: { onmessage: ((event: unknown) => void) | null } },
      ) => {
        args.onEvent.onmessage?.({ type: "progress", received: 4, total: 10 });
        return outcome;
      },
    );

    const events: unknown[] = [];

    await expect(
      startModelDownload(null, "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m", (event: unknown) => events.push(event)),
    ).resolves.toEqual(outcome);

    const [command, args] = invoke.mock.calls[0] as [string, Record<string, unknown>];

    expect(command).toBe("start_model_download");
    expect(Object.keys(args).sort()).toEqual(["onEvent", "profileId", "targetDir"]);
    expect(args.targetDir).toBeNull();
    expect(args.onEvent).toBeInstanceOf(Channel);
    expect(events).toEqual([{ type: "progress", received: 4, total: 10 }]);
  });

  it("sends an explicit download folder when one is chosen", async () => {
    invoke.mockResolvedValue({
      path: null,
      fileName: "m.gguf",
      sizeBytes: 0,
      sha256: "",
      cancelled: true,
    });

    await startModelDownload("D:/Models", "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m", () => {});

    expect(invoke).toHaveBeenCalledWith("start_model_download", {
      targetDir: "D:/Models",
      profileId: "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m",
      onEvent: expect.anything(),
    });
  });

  it("reports a failed cancel as false and a stopped download as cancelled", async () => {
    invoke.mockRejectedValue(new Error("gone"));

    await expect(cancelModelDownload()).resolves.toBe(false);
  });

  it("verifies a chosen file through the registry command", async () => {
    const report = {
      path: "C:/models/a.gguf",
      fileName: "a.gguf",
      profileId: "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m",
      sizeBytes: 12,
      sizeMatches: false,
      sha256: "abc",
      expectedSizeBytes: 4_683_073_536,
      expectedSha256: "509287f7",
      digestMatches: false,
    };

    invoke.mockResolvedValue(report);

    await expect(verifyModel("C:/models/a.gguf")).resolves.toEqual(report);
    expect(invoke).toHaveBeenCalledWith("verify_model", {
      path: "C:/models/a.gguf",
      profileId: null,
    });
  });

  it("passes the model path and context size to validation", async () => {
    invoke.mockResolvedValue({});

    await validateModel("C:/models/a.gguf", 8192);

    expect(invoke).toHaveBeenCalledWith("validate_model", {
      path: "C:/models/a.gguf",
      contextSize: 8192,
    });
  });
});
