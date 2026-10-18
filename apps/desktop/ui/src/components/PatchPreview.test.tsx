import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PatchPreview } from "./PatchPreview";
import type { PatchProposal, PatchValidation } from "../lib/patch";

const proposal: PatchProposal = {
  id: "patch-1",
  path: "src/app.ts",
  originalContent: "let a = 1;\nlet b = 2;\n",
  originalHash: "abc",
  proposedContent: "let a = 1;\nlet b = 3;\n",
  proposedHash: "def",
  createdAt: Date.now(),
};

const okValidation: PatchValidation = {
  ok: true,
  errors: [],
  warnings: [],
  diffSummary: { added: 1, removed: 1, unchanged: 1, originalLines: 2, proposedLines: 2, truncated: false },
};

const invalidValidation: PatchValidation = {
  ok: false,
  errors: ["Sensitive file cannot be patched."],
  warnings: [],
  diffSummary: { added: 0, removed: 0, unchanged: 0, originalLines: 2, proposedLines: 2, truncated: false },
};

// staleValidation kept for reference but not directly rendered in staleOk test
const _staleValidation: PatchValidation = {
  ok: false,
  errors: ["File has changed since proposal was created (stale content)."],
  warnings: ["File has changed since proposal was created (stale content)."],
  diffSummary: { added: 1, removed: 1, unchanged: 1, originalLines: 2, proposedLines: 2, truncated: false },
};
void _staleValidation;

describe("PatchPreview", () => {
  it("shows path and diff summary for valid proposal", () => {
    render(<PatchPreview proposal={proposal} validation={okValidation} onCancel={vi.fn()} onApply={vi.fn()} />);
    expect(screen.getByText("src/app.ts")).toBeTruthy();
    expect(screen.getByText(/\+1 \/ -1/)).toBeTruthy();
    expect(screen.getByText(/let a = 1/)).toBeTruthy();
  });

  it("distinguishes added and removed lines", () => {
    render(<PatchPreview proposal={proposal} validation={okValidation} onCancel={vi.fn()} onApply={vi.fn()} />);
    const added = document.querySelectorAll(".patch-line--added");
    const removed = document.querySelectorAll(".patch-line--removed");
    const unchanged = document.querySelectorAll(".patch-line--unchanged");
    expect(added.length).toBeGreaterThan(0);
    expect(removed.length).toBeGreaterThan(0);
    expect(unchanged.length).toBeGreaterThan(0);
  });

  it("shows horizontal scroll container", () => {
    render(<PatchPreview proposal={proposal} validation={okValidation} onCancel={vi.fn()} onApply={vi.fn()} />);
    const code = document.querySelector(".patch-preview__code");
    expect(code).not.toBeNull();
    expect(code?.getAttribute("tabIndex")).toBe("0");
  });

  it("shows invalid state with errors and cancel", async () => {
    const onCancel = vi.fn();
    render(<PatchPreview proposal={proposal} validation={invalidValidation} onCancel={onCancel} onApply={vi.fn()} />);
    expect(screen.getByText(/Invalid proposal/)).toBeTruthy();
    expect(screen.getByText(/Sensitive file/)).toBeTruthy();
    expect(screen.getByText("Cancel")).toBeTruthy();
    await userEvent.click(screen.getByText("Cancel"));
    expect(onCancel).toHaveBeenCalled();
  });

  it("shows stale warning and disables apply", () => {
    const staleOk: PatchValidation = {
      ok: true,
      errors: [],
      warnings: ["File has changed since proposal was created (stale content)."],
      diffSummary: { added: 1, removed: 1, unchanged: 1, originalLines: 2, proposedLines: 2, truncated: false },
    };
    render(<PatchPreview proposal={proposal} validation={staleOk} onCancel={vi.fn()} onApply={vi.fn()} />);
    expect(screen.getByText(/File has changed/)).toBeTruthy();
    const apply = screen.getByRole("button", { name: /Apply patch/ });
    expect((apply as HTMLButtonElement).disabled).toBe(true);
  });

  it("requires explicit confirm — does not auto-apply", async () => {
    const onApply = vi.fn();
    render(<PatchPreview proposal={proposal} validation={okValidation} onCancel={vi.fn()} onApply={onApply} />);
    expect(onApply).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: /Apply patch/ }));
    expect(onApply).toHaveBeenCalledTimes(1);
  });

  it("shows warning that review does not modify file", () => {
    render(<PatchPreview proposal={proposal} validation={okValidation} onCancel={vi.fn()} onApply={vi.fn()} />);
    expect(screen.getByText(/Reviewing a diff does not modify the file/)).toBeTruthy();
  });

  it("shows loading state", () => {
    render(<PatchPreview proposal={null} validation={null} loading onCancel={vi.fn()} onApply={vi.fn()} />);
    expect(screen.getByText(/Validating proposal/)).toBeTruthy();
  });

  it("shows error state with dismiss", async () => {
    const onCancel = vi.fn();
    render(<PatchPreview proposal={null} validation={null} error="Traversal rejected" onCancel={onCancel} onApply={vi.fn()} />);
    expect(screen.getByText(/Invalid proposal/)).toBeTruthy();
    expect(screen.getByText(/Traversal rejected/)).toBeTruthy();
    await userEvent.click(screen.getByText("Dismiss"));
    expect(onCancel).toHaveBeenCalled();
  });

  it("shows applied state", () => {
    render(<PatchPreview proposal={proposal} validation={null} applied onCancel={vi.fn()} onApply={vi.fn()} />);
    expect(screen.getByText(/Patch applied/)).toBeTruthy();
    expect(screen.getByText(/was updated/)).toBeTruthy();
  });
});
