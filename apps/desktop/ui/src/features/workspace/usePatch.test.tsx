import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { markDesktop } from "../../test/fixtures";

vi.mock("../../lib/patch", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/patch")>();
  return {
    ...actual,
    createPatchProposal: vi.fn(),
    validatePatchProposal: vi.fn(),
    applyPatchProposal: vi.fn(),
    getPatchDiff: vi.fn(),
  };
});

import { applyPatchProposal, createPatchProposal, getPatchDiff, validatePatchProposal } from "../../lib/patch";
import { usePatch } from "./usePatch";

const createMock = vi.mocked(createPatchProposal);
const validateMock = vi.mocked(validatePatchProposal);
const applyMock = vi.mocked(applyPatchProposal);
const diffMock = vi.mocked(getPatchDiff);

const proposal = {
  id: "patch-xyz",
  path: "src/app.ts",
  originalContent: "let a=1\n",
  originalHash: "abc",
  proposedContent: "let a=2\n",
  proposedHash: "def",
  createdAt: Date.now(),
};

beforeEach(() => {
  vi.clearAllMocks();
  markDesktop(true);
  diffMock.mockResolvedValue({ added: 1, removed: 1, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false });
});

describe("usePatch", () => {
  it("starts idle and editing lifecycle", () => {
    const { result } = renderHook(() => usePatch("/tmp/proj"));
    expect(result.current.patchState.status).toBe("idle");
    act(() => result.current.startEditing("a.txt", "hello"));
    expect(result.current.patchState.status).toBe("editing");
    if (result.current.patchState.status === "editing") {
      expect(result.current.patchState.path).toBe("a.txt");
      expect(result.current.editorContent).toBe("hello");
    }
    act(() => result.current.updateEditor("hello world"));
    expect(result.current.editorContent).toBe("hello world");
    act(() => result.current.cancel());
    expect(result.current.patchState.status).toBe("idle");
  });

  it("invalidates on workspace switch", () => {
    const { result, rerender } = renderHook(({ root }) => usePatch(root), { initialProps: { root: "/tmp/a" } });
    act(() => result.current.startEditing("a.txt", "hi"));
    expect(result.current.patchState.status).toBe("editing");
    rerender({ root: "/tmp/b" });
    expect(result.current.patchState.status).toBe("idle");
  });

  it("creates proposal and validates (happy path)", async () => {
    createMock.mockResolvedValue(proposal);
    validateMock.mockResolvedValue({ ok: true, errors: [], warnings: [], diffSummary: { added: 1, removed: 1, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false } });
    const { result } = renderHook(() => usePatch("/tmp/proj"));
    act(() => result.current.startEditing("src/app.ts", "let a=1\n"));
    act(() => result.current.updateEditor("let a=2\n"));
    await act(async () => {
      await result.current.createProposal();
    });
    await waitFor(() => expect(result.current.patchState.status).toBe("preview"));
    if (result.current.patchState.status === "preview") {
      expect(result.current.patchState.proposal.path).toBe("src/app.ts");
      expect(result.current.patchState.validation.ok).toBe(true);
    }
    expect(createMock).toHaveBeenCalledWith("/tmp/proj", "src/app.ts", "let a=2\n");
  });

  it("surfaces validation error (sensitive)", async () => {
    createMock.mockResolvedValue(proposal);
    validateMock.mockResolvedValue({ ok: false, errors: ["Sensitive file cannot be patched."], warnings: [], diffSummary: { added: 0, removed: 0, unchanged: 1, originalLines: 1, proposedLines: 1, truncated: false } });
    const { result } = renderHook(() => usePatch("/tmp/proj"));
    act(() => result.current.startEditing("src/app.ts", "let a=1\n"));
    act(() => result.current.updateEditor("let a=2\n"));
    await act(async () => {
      await result.current.createProposal();
    });
    await waitFor(() => expect(result.current.patchState.status).toBe("preview"));
    if (result.current.patchState.status === "preview") {
      expect(result.current.patchState.validation.ok).toBe(false);
      expect(result.current.patchState.validation.errors[0]).toContain("Sensitive");
    }
  });

  it("handles create error (traversal)", async () => {
    createMock.mockRejectedValue({ code: "forbidden", message: "File escapes workspace." });
    const { result } = renderHook(() => usePatch("/tmp/proj"));
    act(() => result.current.startEditing("../outside.txt", "hi"));
    act(() => result.current.updateEditor("bye"));
    await act(async () => {
      await result.current.createProposal();
    });
    await waitFor(() => expect(result.current.patchState.status).toBe("error"));
    if (result.current.patchState.status === "error") {
      expect(result.current.patchState.message).toContain("escapes");
    }
  });

  it("handles binary rejection", async () => {
    createMock.mockRejectedValue({ code: "binary", message: "Binary files cannot be patched." });
    const { result } = renderHook(() => usePatch("/tmp/proj"));
    act(() => result.current.startEditing("image.png", "binary"));
    act(() => result.current.updateEditor("binary2"));
    await act(async () => {
      await result.current.createProposal();
    });
    await waitFor(() => expect(result.current.patchState.status).toBe("error"));
  });

  it("handles oversized rejection", async () => {
    createMock.mockRejectedValue({ code: "too_large", message: "File too large" });
    const { result } = renderHook(() => usePatch("/tmp/proj"));
    act(() => result.current.startEditing("big.log", "x".repeat(300000)));
    act(() => result.current.updateEditor("y".repeat(300000)));
    await act(async () => {
      await result.current.createProposal();
    });
    expect(result.current.patchState.status).toBe("error");
  });

  it("applies only after explicit confirm and validates", async () => {
    createMock.mockResolvedValue(proposal);
    validateMock.mockResolvedValue({ ok: true, errors: [], warnings: [], diffSummary: { added: 1, removed: 1, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false } });
    applyMock.mockResolvedValue({ applied: true, path: "src/app.ts", previousHash: "abc", newHash: "def", newSize: 10 });
    const { result } = renderHook(() => usePatch("/tmp/proj"));
    act(() => result.current.startEditing("src/app.ts", "let a=1\n"));
    act(() => result.current.updateEditor("let a=2\n"));
    await act(async () => {
      await result.current.createProposal();
    });
    await waitFor(() => expect(result.current.patchState.status).toBe("preview"));
    await act(async () => {
      await result.current.apply();
    });
    await waitFor(() => expect(result.current.patchState.status).toBe("applied"));
    expect(applyMock).toHaveBeenCalledWith("/tmp/proj", proposal);
  });

  it("rejects stale on apply", async () => {
    createMock.mockResolvedValue(proposal);
    validateMock.mockResolvedValue({ ok: true, errors: [], warnings: ["stale"], diffSummary: { added: 1, removed: 1, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false } });
    const { result } = renderHook(() => usePatch("/tmp/proj"));
    act(() => result.current.startEditing("src/app.ts", "let a=1\n"));
    act(() => result.current.updateEditor("let a=2\n"));
    await act(async () => {
      await result.current.createProposal();
    });
    // Simulate stale warning on preview
    // Apply should error if stale
    await act(async () => {
      await result.current.apply();
    });
    await waitFor(() => expect(result.current.patchState.status).toBe("error"));
  });

  it("cancel dismisses preview", async () => {
    createMock.mockResolvedValue(proposal);
    validateMock.mockResolvedValue({ ok: true, errors: [], warnings: [], diffSummary: { added: 1, removed: 1, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false } });
    const { result } = renderHook(() => usePatch("/tmp/proj"));
    act(() => result.current.startEditing("src/app.ts", "let a=1\n"));
    act(() => result.current.updateEditor("let a=2\n"));
    await act(async () => {
      await result.current.createProposal();
    });
    await waitFor(() => expect(result.current.patchState.status).toBe("preview"));
    act(() => result.current.cancel());
    expect(result.current.patchState.status).toBe("idle");
  });

  it("never auto-applies on preview — explicit only", async () => {
    createMock.mockResolvedValue(proposal);
    validateMock.mockResolvedValue({ ok: true, errors: [], warnings: [], diffSummary: { added: 1, removed: 1, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false } });
    const { result } = renderHook(() => usePatch("/tmp/proj"));
    act(() => result.current.startEditing("src/app.ts", "let a=1\n"));
    act(() => result.current.updateEditor("let a=2\n"));
    await act(async () => {
      await result.current.createProposal();
    });
    // After preview, apply should not have been called automatically
    expect(applyMock).not.toHaveBeenCalled();
  });

  it("multi-file rejected is surfaced as error (model tried to send 2 files)", async () => {
    // Simulate backend rejecting multi-file proposal — frontend only allows single file, so this error is from invoke
    createMock.mockRejectedValue({ code: "invalid_request", message: "Only single-file patches are allowed." });
    const { result } = renderHook(() => usePatch("/tmp/proj"));
    act(() => result.current.startEditing("a.txt", "hi"));
    act(() => result.current.updateEditor("bye"));
    await act(async () => {
      await result.current.createProposal();
    });
    expect(result.current.patchState.status).toBe("error");
  });
});
