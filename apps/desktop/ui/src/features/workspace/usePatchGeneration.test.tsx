import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { markDesktop } from "../../test/fixtures";

vi.mock("../../lib/inference", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/inference")>();
  return {
    ...actual,
    generate: vi.fn(),
    cancelGeneration: vi.fn().mockResolvedValue(true),
  };
});

vi.mock("../../lib/patch", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/patch")>();
  return {
    ...actual,
    createPatchProposal: vi.fn(),
    validatePatchProposal: vi.fn(),
    applyPatchProposal: vi.fn(),
    createPatchBatch: vi.fn(),
    validatePatchBatch: vi.fn(),
    applyPatchBatch: vi.fn(),
    getPatchDiff: vi.fn().mockResolvedValue({ added: 1, removed: 1, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false }),
  };
});

import { generate, cancelGeneration } from "../../lib/inference";
import {
  createPatchProposal,
  validatePatchProposal,
  applyPatchProposal,
  createPatchBatch,
  validatePatchBatch,
  applyPatchBatch,
} from "../../lib/patch";
import { usePatchGeneration } from "./usePatchGeneration";

const generateMock = vi.mocked(generate);
const cancelMock = vi.mocked(cancelGeneration);
const createMock = vi.mocked(createPatchProposal);
const validateMock = vi.mocked(validatePatchProposal);
const applyMock = vi.mocked(applyPatchProposal);
const createBatchMock = vi.mocked(createPatchBatch);
const validateBatchMock = vi.mocked(validatePatchBatch);
const applyBatchMock = vi.mocked(applyPatchBatch);

const proposal = {
  id: "patch-1",
  path: "src/app.ts",
  originalContent: "let a = 1;\n",
  originalHash: "abc",
  proposedContent: "let a = 2;\n",
  proposedHash: "def",
  createdAt: Date.now(),
};

function appStatus(state: string) {
  return {
    name: "Nexora",
    version: "0.1.0",
    engine: { state, runtime: state === "ready" ? "llama-server" : null, model: state === "ready" ? "model.gguf" : null, detail: state },
    modelPath: state === "ready" ? "/tmp/model.gguf" : null,
    runtimePath: state === "ready" ? "/tmp/llama-server" : null,
    contextSize: 32768,
  } as unknown as import("../../lib/ipc").AppStatus;
}

beforeEach(() => {
  vi.clearAllMocks();
  markDesktop(true);
  generateMock.mockReset();
  cancelMock.mockResolvedValue(true);
  createMock.mockResolvedValue(proposal);
  validateMock.mockResolvedValue({ ok: true, errors: [], warnings: [], diffSummary: { added: 1, removed: 1, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false } });
  applyMock.mockResolvedValue({ applied: true, path: "src/app.ts", previousHash: "abc", newHash: "def", newSize: 10 });
  createBatchMock.mockReset();
  validateBatchMock.mockReset();
  applyBatchMock.mockReset();
});

describe("usePatchGeneration — inference integration", () => {
  it("rejects when model unavailable", async () => {
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("not_configured") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    expect(result.current.generationState.status).toBe("error");
    if (result.current.generationState.status === "error") {
      expect(result.current.generationState.message).toMatch(/Model unavailable/i);
    }
    expect(generateMock).not.toHaveBeenCalled();
  });

  it("rejects when workspace not selected", async () => {
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: null, engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    expect(result.current.generationState.status).toBe("error");
    if (result.current.generationState.status === "error") {
      expect(result.current.generationState.message).toMatch(/No project/i);
    }
  });

  it("rejects when no file selected", async () => {
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    expect(result.current.generationState.status).toBe("error");
    expect(result.current.generationState.status).toBe("error");
  });

  it("rejects sensitive file", async () => {
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: ".env", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    expect(result.current.generationState.status).toBe("error");
    if (result.current.generationState.status === "error") {
      expect(result.current.generationState.message).toMatch(/sensitive/i);
    }
  });

  it("handles successful streaming → preview", async () => {
    generateMock.mockImplementation(async (req, onEvent) => {
      // Simulate streaming deltas
      onEvent({ type: "delta", requestId: req.requestId, text: '{"version":1,"targetPath":"a.txt","proposedContent":"new"}' });
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "change it", filteredContextBlock: "context" });
    });
    await waitFor(() => expect(result.current.generationState.status).toBe("preview"));
    if (result.current.generationState.status === "preview") {
      expect(result.current.generationState.proposal.path).toBe("src/app.ts");
      expect(result.current.generationState.validation.ok).toBe(true);
    }
    expect(createMock).toHaveBeenCalledWith("/tmp/proj", "a.txt", "new");
  });

  it("handles runtime error during generation", async () => {
    generateMock.mockRejectedValue({ code: "runtime_error", message: "Engine failed" });
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    expect(result.current.generationState.status).toBe("error");
    if (result.current.generationState.status === "error") {
      expect(result.current.generationState.message).toMatch(/Engine failed/);
    }
  });

  it("handles malformed response during streaming", async () => {
    generateMock.mockImplementation(async (req, onEvent) => {
      onEvent({ type: "delta", requestId: req.requestId, text: "not json at all" });
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    await waitFor(() => expect(result.current.generationState.status).toBe("error"));
    if (result.current.generationState.status === "error") {
      expect(result.current.generationState.message).toMatch(/Malformed|valid JSON/i);
    }
  });

  it("supports cancellation", async () => {
    generateMock.mockImplementation(async (req) => {
      await new Promise((resolve) => setTimeout(resolve, 50));
      return { requestId: req.requestId, finishReason: null, cancelled: true };
    });
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    let genPromise: Promise<void> | null = null;
    await act(async () => {
      genPromise = result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(result.current.generationState.status).toBe("generating");
    await act(async () => {
      await result.current.cancel();
    });
    expect(cancelMock).toHaveBeenCalled();
    if (genPromise) await act(async () => { await genPromise; });
    expect(result.current.generationState.status).toBe("cancelled");
  });

  it("prevents duplicate submission while generating", async () => {
    generateMock.mockImplementation(async (req) => {
      await new Promise((r) => setTimeout(r, 50));
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    let firstPromise: Promise<void> | null = null;
    await act(async () => {
      firstPromise = result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "first", filteredContextBlock: null });
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(result.current.generationState.status).toBe("generating");
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "second", filteredContextBlock: null });
    });
    expect(result.current.generationState.status).toBe("error");
    if (result.current.generationState.status === "error") {
      expect(result.current.generationState.message).toMatch(/already in progress/i);
    }
    expect(generateMock).toHaveBeenCalledTimes(1);
    if (firstPromise) {
      await act(async () => {
        await firstPromise;
        await new Promise((r) => setTimeout(r, 60));
      });
    }
  });

  it("respects context-budget — uses filteredContextBlock only, not unbounded", async () => {
    // This test ensures generation uses only filtered block, not full unbouded context
    // We check that generate is called with messages containing filtered block, not oversized
    let capturedMessages: unknown = null;
    generateMock.mockImplementation(async (req, onEvent) => {
      capturedMessages = (req as { messages: unknown }).messages;
      onEvent({ type: "delta", requestId: req.requestId, text: '{"version":1,"targetPath":"a.txt","proposedContent":"new"}' });
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
    const filtered = "File: a.txt\n```ts\ncontent\n```\n"; // budgeted
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: filtered });
    });
    expect(capturedMessages).toBeTruthy();
    const msgs = capturedMessages as { content: string }[];
    const combined = msgs.map((m) => m.content).join("\n");
    expect(combined).toContain(filtered);
    // Should not contain oversized unbounded context (we didn't pass it)
    expect(combined.length).toBeLessThan(5000);
  });

  it("never falls back to external provider — error on desktop_only", async () => {
    markDesktop(false);
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    // Should be error about desktop app, not external API call
    expect(result.current.generationState.status).toBe("error");
    if (result.current.generationState.status === "error") {
      expect(result.current.generationState.message).toMatch(/desktop app|Model unavailable/i);
    }
    // Ensure generate not called when not desktop? Actually our hook checks isDesktopShell before generate, so generate not called
    // But if it were, it would throw desktop_only, not external
    markDesktop(true);
  });

  it("invalidates on workspace switch", async () => {
    generateMock.mockImplementation(async (req, onEvent) => {
      onEvent({ type: "delta", requestId: req.requestId, text: '{"version":1,"targetPath":"a.txt","proposedContent":"new"}' });
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
    const { result, rerender } = renderHook(({ ws }) => usePatchGeneration({ workspacePath: ws, engineStatus: appStatus("ready") }), { initialProps: { ws: "/tmp/a" } });
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    await waitFor(() => expect(result.current.generationState.status).toBe("preview"));
    // Switch workspace
    rerender({ ws: "/tmp/b" });
    expect(result.current.generationState.status).toBe("idle");
  });

  it("validation failure does not reach apply", async () => {
    generateMock.mockImplementation(async (req, onEvent) => {
      onEvent({ type: "delta", requestId: req.requestId, text: '{"version":1,"targetPath":"a.txt","proposedContent":"new"}' });
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
    validateMock.mockResolvedValue({ ok: false, errors: ["Sensitive file"], warnings: [], diffSummary: { added: 0, removed: 0, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false } });
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    expect(result.current.generationState.status).toBe("error");
    expect(applyMock).not.toHaveBeenCalled();
  });

  it("dismissal never writes", async () => {
    generateMock.mockImplementation(async (req, onEvent) => {
      onEvent({ type: "delta", requestId: req.requestId, text: '{"version":1,"targetPath":"a.txt","proposedContent":"new"}' });
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    await waitFor(() => expect(result.current.generationState.status).toBe("preview"));
    await act(async () => {
      result.current.dismiss();
    });
    expect(result.current.generationState.status).toBe("idle");
    expect(applyMock).not.toHaveBeenCalled();
  });

  it("requires explicit confirmation — no auto-apply", async () => {
    generateMock.mockImplementation(async (req, onEvent) => {
      onEvent({ type: "delta", requestId: req.requestId, text: '{"version":1,"targetPath":"a.txt","proposedContent":"new"}' });
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    await waitFor(() => expect(result.current.generationState.status).toBe("preview"));
    expect(applyMock).not.toHaveBeenCalled();
  });

  it("apply success after preview", async () => {
    generateMock.mockImplementation(async (req, onEvent) => {
      onEvent({ type: "delta", requestId: req.requestId, text: '{"version":1,"targetPath":"a.txt","proposedContent":"new"}' });
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    await waitFor(() => expect(result.current.generationState.status).toBe("preview"));
    await act(async () => {
      await result.current.apply();
    });
    expect(applyMock).toHaveBeenCalledWith("/tmp/proj", proposal);
    expect(result.current.generationState.status).toBe("applied");
  });

  it("failed validation or apply never reports false success", async () => {
    generateMock.mockImplementation(async (req, onEvent) => {
      onEvent({ type: "delta", requestId: req.requestId, text: '{"version":1,"targetPath":"a.txt","proposedContent":"new"}' });
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
    applyMock.mockRejectedValue({ code: "stale", message: "File changed" });
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "fix", filteredContextBlock: null });
    });
    await waitFor(() => expect(result.current.generationState.status).toBe("preview"));
    await act(async () => {
      await result.current.apply();
    });
    expect(result.current.generationState.status).toBe("error");
    if (result.current.generationState.status === "error") {
      expect(result.current.generationState.message).toMatch(/File changed/);
    }
  });
});

describe("usePatchGeneration — multi-file batches (contract v2)", () => {
  const batchFixture = {
    id: "batch-1",
    files: [proposal, { ...proposal, id: "patch-2", path: "src/b.ts" }],
    createdAt: Date.now(),
  };
  const batchValidationOk = {
    ok: true,
    errors: [],
    warnings: [],
    files: [
      { path: "src/app.ts", validation: { ok: true, errors: [], warnings: [], diffSummary: { added: 1, removed: 1, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false } } },
      { path: "src/b.ts", validation: { ok: true, errors: [], warnings: [], diffSummary: { added: 1, removed: 1, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false } } },
    ],
  };

  function streamV2(files: Array<Record<string, unknown>>) {
    generateMock.mockImplementation(async (req, onEvent) => {
      onEvent({
        type: "delta",
        requestId: req.requestId,
        text: JSON.stringify({ version: 2, explanation: "multi", files }),
      });
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
  }

  it("turns v2 multi-file output into an atomic batch preview", async () => {
    streamV2([
      { targetPath: "a.txt", proposedContent: "a2" },
      { targetPath: "b.txt", proposedContent: "b2" },
    ]);
    createBatchMock.mockResolvedValue(batchFixture as never);
    validateBatchMock.mockResolvedValue(batchValidationOk as never);

    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "change both", filteredContextBlock: null });
    });

    await waitFor(() => expect(result.current.generationState.status).toBe("previewBatch"));
    expect(createBatchMock).toHaveBeenCalledWith("/tmp/proj", [
      { relativePath: "a.txt", proposedContent: "a2" },
      { relativePath: "b.txt", proposedContent: "b2" },
    ]);
    if (result.current.generationState.status === "previewBatch") {
      expect(result.current.generationState.batch.files).toHaveLength(2);
      expect(result.current.generationState.explanation).toBe("multi");
    }
  });

  it("applies a batch atomically via apply_patch_batch", async () => {
    streamV2([
      { targetPath: "a.txt", proposedContent: "a2" },
      { targetPath: "b.txt", proposedContent: "b2" },
    ]);
    createBatchMock.mockResolvedValue(batchFixture as never);
    validateBatchMock.mockResolvedValue(batchValidationOk as never);
    applyBatchMock.mockResolvedValue({ applied: true, results: [] } as never);

    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "change both", filteredContextBlock: null });
    });
    await waitFor(() => expect(result.current.generationState.status).toBe("previewBatch"));
    await act(async () => {
      await result.current.apply();
    });
    await waitFor(() => expect(result.current.generationState.status).toBe("appliedBatch"));
    expect(applyBatchMock).toHaveBeenCalledWith("/tmp/proj", batchFixture);
  });

  it("surfaces batch validation failures as honest errors", async () => {
    streamV2([
      { targetPath: "a.txt", proposedContent: "a2" },
      { targetPath: "b.txt", proposedContent: "b2" },
    ]);
    createBatchMock.mockResolvedValue(batchFixture as never);
    validateBatchMock.mockResolvedValue({
      ok: false,
      errors: ["src/b.ts: Stale proposal: file has changed."],
      warnings: [],
      files: [],
    } as never);

    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "change both", filteredContextBlock: null });
    });
    await waitFor(() => expect(result.current.generationState.status).toBe("error"));
    if (result.current.generationState.status === "error") {
      expect(result.current.generationState.message).toMatch(/Stale proposal/);
    }
  });

  it("keeps v1 single-file output on the unchanged single path", async () => {
    generateMock.mockImplementation(async (req, onEvent) => {
      onEvent({ type: "delta", requestId: req.requestId, text: '{"version":1,"targetPath":"a.txt","proposedContent":"new"}' });
      return { requestId: req.requestId, finishReason: "stop", cancelled: false };
    });
    const { result } = renderHook(() => usePatchGeneration({ workspacePath: "/tmp/proj", engineStatus: appStatus("ready") }));
    await act(async () => {
      await result.current.generatePatch({ targetPath: "a.txt", originalContent: "old", instruction: "change it", filteredContextBlock: null });
    });
    await waitFor(() => expect(result.current.generationState.status).toBe("preview"));
    expect(createMock).toHaveBeenCalledWith("/tmp/proj", "a.txt", "new");
    expect(createBatchMock).not.toHaveBeenCalled();
  });
});
