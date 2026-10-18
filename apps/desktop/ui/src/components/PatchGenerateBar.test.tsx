import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PatchGenerateBar } from "./PatchGenerateBar";
import type { AppStatus } from "../lib/ipc";

const mockGeneratePatch = vi.fn();
const mockCancel = vi.fn();
const mockDismiss = vi.fn();
const mockApply = vi.fn();
const mockInvalidate = vi.fn();

let mockState: { status: string; [key: string]: unknown } = { status: "idle" };

vi.mock("../features/workspace/usePatchGeneration", () => ({
  usePatchGeneration: () => ({
    generationState: mockState,
    generatePatch: mockGeneratePatch,
    cancel: mockCancel,
    dismiss: mockDismiss,
    apply: mockApply,
    invalidate: mockInvalidate,
  }),
}));

function appStatus(state: string): AppStatus {
  return {
    name: "Nexora",
    version: "0.1.0",
    engine: { state: state as AppStatus["engine"]["state"], runtime: state === "ready" ? "r" : null, model: state === "ready" ? "m" : null, detail: state },
    modelPath: state === "ready" ? "/tmp/m.gguf" : null,
    runtimePath: state === "ready" ? "/tmp/r" : null,
    contextSize: 32768,
  } as AppStatus;
}

beforeEach(() => {
  vi.clearAllMocks();
  mockState = { status: "idle" };
});

describe("PatchGenerateBar — states", () => {
  it("shows no workspace", () => {
    render(<PatchGenerateBar workspacePath={null} targetPath={null} originalContent={null} filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    expect(screen.getByText(/Open a project/i)).toBeTruthy();
  });

  it("shows no file selected", () => {
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath={null} originalContent={null} filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    expect(screen.getByText(/Select one eligible file/i)).toBeTruthy();
  });

  it("shows file excluded", () => {
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath=".env" originalContent="secret" filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    expect(screen.getByText(/excluded for security/i)).toBeTruthy();
  });

  it("shows no model available", () => {
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock={null} engineStatus={appStatus("not_configured")} />);
    expect(screen.getByText(/No model available/i)).toBeTruthy();
    expect(screen.getByText(/Generate patch/)).toBeTruthy();
    expect(screen.getByRole("button", { name: /Generate patch/ })).toBeTruthy();
    // Button should be disabled when no model
    expect((screen.getByRole("button", { name: /Generate patch/ }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("shows model loading", () => {
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock={null} engineStatus={appStatus("loading")} />);
    expect(screen.getByText(/Model is loading/i)).toBeTruthy();
  });

  it("shows generating state with cancel", async () => {
    mockState = { status: "generating", requestId: "req-1", collected: '{"version":1' };
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    expect(screen.getByText(/Generating patch via local model/i)).toBeTruthy();
    expect(screen.getByText(/Cancel/)).toBeTruthy();
    // Also shows generating via hook state
    expect(screen.getByText(/Generating…/)).toBeTruthy();
  });

  it("shows cancelled state", () => {
    mockState = { status: "cancelled", collected: "partial" };
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    expect(screen.getByText(/Generation cancelled/i)).toBeTruthy();
  });

  it("shows malformed / validation error with retry", () => {
    mockState = { status: "error", message: "Malformed patch JSON: unexpected" };
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    expect(screen.getByText(/Malformed patch JSON/i)).toBeTruthy();
    expect(screen.getByText(/Retry/)).toBeTruthy();
  });

  it("shows staleness error", () => {
    mockState = { status: "error", message: "File changed since proposal creation" };
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    expect(screen.getByText(/File changed/i)).toBeTruthy();
  });

  it("shows preview with explanation and diff", () => {
    const proposal = {
      id: "patch-1",
      path: "a.txt",
      originalContent: "old",
      originalHash: "abc",
      proposedContent: "new",
      proposedHash: "def",
      createdAt: Date.now(),
    };
    mockState = {
      status: "preview",
      proposal,
      validation: { ok: true, errors: [], warnings: [], diffSummary: { added: 1, removed: 1, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false } },
      explanation: "Fixes bug",
      collected: '{"version":1}',
    };
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    expect(screen.getByText(/Fixes bug/)).toBeTruthy();
    expect(screen.getByText("a.txt")).toBeTruthy(); // from PatchPreview
    expect(screen.getByText(/Reviewing a diff does not modify/i)).toBeTruthy();
  });

  it("shows applied success", () => {
    const proposal = {
      id: "patch-1",
      path: "a.txt",
      originalContent: "old",
      originalHash: "abc",
      proposedContent: "new",
      proposedHash: "def",
      createdAt: Date.now(),
    };
    mockState = { status: "applied", proposal, result: { applied: true, path: "a.txt", previousHash: "abc", newHash: "def", newSize: 3 }, explanation: "done" };
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    expect(screen.getByText(/Patch applied/i)).toBeTruthy();
  });

  it("prevents duplicate while generating — button disabled", () => {
    mockState = { status: "generating", requestId: "1", collected: "" };
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    expect(screen.queryByRole("button", { name: /Generate patch/ })).toBeNull(); // not shown when generating, shows Cancel
    expect(screen.getByText(/Cancel/)).toBeTruthy();
  });

  it("calls generate with instruction", async () => {
    mockState = { status: "idle" };
    const user = userEvent.setup();
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock="context" engineStatus={appStatus("ready")} />);
    const textarea = screen.getByLabelText(/Change request for a.txt/i);
    await user.type(textarea, "refactor to async");
    await user.click(screen.getByRole("button", { name: /Generate patch/ }));
    expect(mockGeneratePatch).toHaveBeenCalledWith(
      expect.objectContaining({
        targetPath: "a.txt",
        originalContent: "old",
        instruction: "refactor to async",
        filteredContextBlock: "context",
      }),
    );
  });

  it("dismiss never writes — calls dismiss", async () => {
    mockState = { status: "error", message: "Malformed" };
    const user = userEvent.setup();
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    await user.click(screen.getByText(/Dismiss/));
    expect(mockDismiss).toHaveBeenCalled();
  });

  it("requires explicit confirmation — preview shows Apply", () => {
    const proposal = {
      id: "patch-1",
      path: "a.txt",
      originalContent: "old",
      originalHash: "abc",
      proposedContent: "new",
      proposedHash: "def",
      createdAt: Date.now(),
    };
    mockState = {
      status: "preview",
      proposal,
      validation: { ok: true, errors: [], warnings: [], diffSummary: { added: 1, removed: 0, unchanged: 0, originalLines: 1, proposedLines: 1, truncated: false } },
      explanation: "exp",
      collected: "",
    };
    render(<PatchGenerateBar workspacePath="/tmp/proj" targetPath="a.txt" originalContent="old" filteredContextBlock={null} engineStatus={appStatus("ready")} />);
    const apply = screen.getByRole("button", { name: /Apply patch/ });
    expect(apply).toBeTruthy();
    // No auto-apply on render
    expect(mockApply).not.toHaveBeenCalled();
  });
});
