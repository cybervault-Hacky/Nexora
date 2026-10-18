import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { markDesktop } from "../../test/fixtures";

vi.mock("../../lib/validation", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/validation")>();
  return {
    ...actual,
    validatePatchDetailed: vi.fn(),
  };
});

import { validatePatchDetailed } from "../../lib/validation";
import { useValidation } from "./useValidation";
import type { PatchProposal } from "../../lib/patch";
import type { ValidationReport } from "../../lib/validation";

const validateMock = vi.mocked(validatePatchDetailed);

function proposal(overrides: Partial<PatchProposal> = {}): PatchProposal {
  return {
    id: "patch-1",
    path: "src/app.ts",
    originalContent: "old",
    originalHash: "oldhash",
    proposedContent: "new",
    proposedHash: "newhash",
    createdAt: Date.now(),
    ...overrides,
  };
}

function report(overrides: Partial<ValidationReport> = {}): ValidationReport {
  return {
    overall: "passed",
    checks: [
      { id: "contract_schema", label: "Patch contract & schema", status: "passed", diagnostics: [], note: null },
      { id: "syntax", label: "Syntax (.ts)", status: "passed", diagnostics: [], note: null },
    ],
    summary: "All checks passed",
    durationMs: 5,
    proposalId: "patch-1",
    proposalPath: "src/app.ts",
    generatedAt: Date.now(),
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  validateMock.mockReset();
  markDesktop(true);
  vi.useRealTimers();
});

describe("useValidation", () => {
  it("validates successfully and associates report", async () => {
    const p = proposal();
    const r = report({ proposalId: p.id });
    validateMock.mockResolvedValue(r);
    const { result } = renderHook(() => useValidation("/tmp/proj"));
    await act(async () => {
      await result.current.validate(p);
    });
    await waitFor(() => expect(result.current.validationState.status).toBe("validated"));
    const state = result.current.validationState as { status: "validated"; report: ValidationReport };
    expect(state.report.proposalId).toBe(p.id);
    expect(state.report.overall).toBe("passed");
  });

  it("reports failed validation with diagnostics", async () => {
    const p = proposal();
    const r = report({
      overall: "failed",
      checks: [
        { id: "syntax", label: "Syntax", status: "failed", diagnostics: [{ path: "src/app.ts", line: 3, column: 5, message: "Mismatched" }], note: null },
      ],
      proposalId: p.id,
    });
    validateMock.mockResolvedValue(r);
    const { result } = renderHook(() => useValidation("/tmp/proj"));
    await act(async () => {
      await result.current.validate(p);
    });
    await waitFor(() => expect(result.current.validationState.status).toBe("validated"));
    const state = result.current.validationState as { status: "validated"; report: ValidationReport };
    expect(state.report.overall).toBe("failed");
    expect(state.report.checks[0]!.diagnostics[0]!.path).toBe("src/app.ts");
  });

  it("reports unsupported and notRun checks", async () => {
    const p = proposal({ path: "README.md" });
    const r = report({
      checks: [
        { id: "syntax", label: "Syntax (.md)", status: "unsupported", diagnostics: [], note: "No structural check for .md" },
        { id: "static_analysis", label: "Static analysis", status: "notRun", diagnostics: [], note: "Not run — would require executing" },
      ],
      proposalId: p.id,
    });
    validateMock.mockResolvedValue(r);
    const { result } = renderHook(() => useValidation("/tmp/proj"));
    await act(async () => {
      await result.current.validate(p);
    });
    await waitFor(() => expect(result.current.validationState.status).toBe("validated"));
    const state = result.current.validationState as any;
    expect(state.report.checks.some((c: any) => c.status === "unsupported")).toBe(true);
    expect(state.report.checks.some((c: any) => c.status === "notRun")).toBe(true);
  });

  it("handles invalid contract via error", async () => {
    const p = proposal({ path: "../outside.txt" });
    validateMock.mockRejectedValue({ code: "invalid_request", message: "Invalid target path" });
    const { result } = renderHook(() => useValidation("/tmp/proj"));
    await act(async () => {
      await result.current.validate(p);
    });
    await waitFor(() => expect(result.current.validationState.status).toBe("error"));
    expect((result.current.validationState as any).message).toContain("Invalid target path");
  });

  it("handles sensitive path failure", async () => {
    const p = proposal({ path: ".env" });
    const r = report({
      overall: "failed",
      checks: [{ id: "sensitive_binary_size", label: "Sensitive", status: "failed", diagnostics: [{ path: ".env", line: 1, message: "Sensitive file" }], note: null }],
      proposalId: p.id,
    });
    validateMock.mockResolvedValue(r);
    const { result } = renderHook(() => useValidation("/tmp/proj"));
    await act(async () => {
      await result.current.validate(p);
    });
    await waitFor(() => expect(result.current.validationState.status).toBe("validated"));
    expect((result.current.validationState as any).report.overall).toBe("failed");
  });

  it("detects stale proposal (association)", async () => {
    const p1 = proposal({ id: "patch-1", path: "a.txt" });
    const p2 = proposal({ id: "patch-2", path: "b.txt" });
    const r1 = report({ proposalId: "patch-1", proposalPath: "a.txt" });
    const r2 = report({ proposalId: "patch-2", proposalPath: "b.txt" });
    validateMock.mockImplementation(async (_root, prop) => {
      // Simulate delay for p1
      if ((prop as PatchProposal).id === "patch-1") {
        await new Promise((res) => setTimeout(res, 50));
        return r1;
      }
      return r2;
    });
    const { result } = renderHook(() => useValidation("/tmp/proj"));
    // Start p1 then quickly p2 — stale result for p1 should be ignored
    act(() => {
      void result.current.validate(p1);
    });
    await act(async () => {
      await result.current.validate(p2);
    });
    await waitFor(() => expect(result.current.validationState.status).toBe("validated"));
    // Should be p2's report, not p1's
    const state = result.current.validationState as any;
    expect(state.report.proposalId).toBe("patch-2");
  });

  it("handles cancellation", async () => {
    const p = proposal();
    validateMock.mockImplementation(() => new Promise(() => {})); // never resolves
    const { result } = renderHook(() => useValidation("/tmp/proj"));
    act(() => {
      void result.current.validate(p);
    });
    await waitFor(() => expect(result.current.validationState.status).toBe("validating"));
    await act(async () => {
      await result.current.cancel();
    });
    expect(result.current.validationState.status).toBe("cancelled");
  });

  it("handles timeout", async () => {
    vi.useFakeTimers();
    const p = proposal();
    validateMock.mockImplementation(() => new Promise(() => {}));
    const { result } = renderHook(() => useValidation("/tmp/proj"));
    act(() => {
      void result.current.validate(p);
    });
    expect(result.current.validationState.status).toBe("validating");
    act(() => {
      vi.advanceTimersByTime(3100);
    });
    expect(result.current.validationState.status).toBe("error");
    expect((result.current.validationState as any).message).toContain("timed out");
    vi.useRealTimers();
  });

  it("resets on workspace switch", async () => {
    const p = proposal();
    const r = report({ proposalId: p.id });
    validateMock.mockResolvedValue(r);
    const { result, rerender } = renderHook(({ root }) => useValidation(root), { initialProps: { root: "/tmp/proj" as string | null } });
    await act(async () => {
      await result.current.validate(p);
    });
    await waitFor(() => expect(result.current.validationState.status).toBe("validated"));
    // Switch workspace
    rerender({ root: "/tmp/other" });
    expect(result.current.validationState.status).toBe("idle");
  });

  it("ensures diagnostics associated with correct proposal", async () => {
    const p = proposal({ id: "patch-99" });
    const r = report({
      proposalId: "patch-99",
      checks: [{ id: "syntax", label: "Syntax", status: "failed", diagnostics: [{ path: "src/app.ts", line: 10, column: 2, message: "Unclosed" }], note: null }],
    });
    validateMock.mockResolvedValue(r);
    const { result } = renderHook(() => useValidation("/tmp/proj"));
    await act(async () => {
      await result.current.validate(p);
    });
    await waitFor(() => expect(result.current.validationState.status).toBe("validated"));
    const state = result.current.validationState as any;
    expect(state.report.proposalId).toBe("patch-99");
    expect(state.report.checks[0].diagnostics[0].line).toBe(10);
  });
});
