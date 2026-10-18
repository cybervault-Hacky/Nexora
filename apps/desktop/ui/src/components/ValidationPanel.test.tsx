import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ValidationPanel } from "./ValidationPanel";
import type { ValidationReport } from "../lib/validation";
import type { PatchProposal } from "../lib/patch";

function proposal(): PatchProposal {
  return {
    id: "patch-1",
    path: "src/app.ts",
    originalContent: "old",
    originalHash: "oldhash",
    proposedContent: "new",
    proposedHash: "newhash",
    createdAt: Date.now(),
  };
}

function report(overrides: Partial<ValidationReport> = {}): ValidationReport {
  return {
    overall: "failed",
    checks: [
      { id: "contract_schema", label: "Patch contract & schema", status: "passed", diagnostics: [], note: null },
      { id: "syntax", label: "Syntax (.ts)", status: "failed", diagnostics: [{ path: "src/app.ts", line: 3, column: 5, message: "Unclosed {" }], note: "Heuristic" },
      { id: "static_analysis", label: "Static analysis", status: "notRun", diagnostics: [], note: "Not run — would require executing" },
      { id: "target_boundary", label: "Target", status: "unsupported", diagnostics: [], note: "No check for .md" },
    ],
    summary: "Validation failed",
    durationMs: 12,
    proposalId: "patch-1",
    proposalPath: "src/app.ts",
    generatedAt: Date.now(),
    ...overrides,
  };
}

describe("ValidationPanel", () => {
  it("renders overall status and checks with diagnostics", () => {
    const r = report();
    render(
      <ValidationPanel
        proposal={proposal()}
        report={r}
        validating={false}
        error={null}
        onValidate={vi.fn()}
        repairState={null}
        onExplain={vi.fn()}
        onRepair={vi.fn()}
        maxAttempts={3}
      />,
    );
    expect(screen.getByLabelText("Validation")).toBeTruthy();
    expect(screen.getByText(/Validation failed/)).toBeTruthy();
    expect(screen.getAllByText(/Patch contract/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/Unclosed/)).toBeTruthy();
    expect(screen.getByText(/src\/app.ts:3:5/)).toBeTruthy();
    expect(screen.getAllByText(/Heuristic/).length).toBeGreaterThanOrEqual(1);
  });

  it("shows notRun and unsupported with reasons", () => {
    const r = report();
    render(
      <ValidationPanel
        proposal={proposal()}
        report={r}
        validating={false}
        error={null}
        onValidate={vi.fn()}
        repairState={null}
        onExplain={vi.fn()}
        onRepair={vi.fn()}
        maxAttempts={3}
      />,
    );
    expect(screen.getAllByText(/Not run/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText(/Not run — would require executing/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText(/unsupported/i).length).toBeGreaterThanOrEqual(1);
  });

  it("shows what was checked / not checked", () => {
    const r = report();
    render(
      <ValidationPanel
        proposal={proposal()}
        report={r}
        validating={false}
        error={null}
        onValidate={vi.fn()}
        repairState={null}
        onExplain={vi.fn()}
        onRepair={vi.fn()}
        maxAttempts={3}
      />,
    );
    expect(screen.getByText(/What was checked/)).toBeTruthy();
  });

  it("shows Validate button and handles loading", async () => {
    const user = userEvent.setup();
    const onValidate = vi.fn();
    render(
      <ValidationPanel
        proposal={proposal()}
        report={null}
        validating={false}
        error={null}
        onValidate={onValidate}
        repairState={null}
        onExplain={vi.fn()}
        onRepair={vi.fn()}
        maxAttempts={3}
      />,
    );
    expect(screen.getByText(/never writes into your project/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Validate" }));
    expect(onValidate).toHaveBeenCalled();

    render(
      <ValidationPanel
        proposal={proposal()}
        report={null}
        validating={true}
        error={null}
        onValidate={vi.fn()}
        onCancelValidate={vi.fn()}
        repairState={null}
        onExplain={vi.fn()}
        onRepair={vi.fn()}
        maxAttempts={3}
      />,
    );
    expect(screen.getByText(/Validating/)).toBeTruthy();
  });

  it("shows error, timeout, cancellation", () => {
    render(
      <ValidationPanel
        proposal={proposal()}
        report={null}
        validating={false}
        error="Validation timed out"
        onValidate={vi.fn()}
        repairState={{ status: "cancelled" } as any}
        onExplain={vi.fn()}
        onRepair={vi.fn()}
        maxAttempts={3}
      />,
    );
    expect(screen.getByText(/Validation timed out/)).toBeTruthy();
    expect(screen.getByText(/Repair cancelled/)).toBeTruthy();
  });

  it("shows Explain and Propose fix buttons and respects limits", async () => {
    const user = userEvent.setup();
    const onExplain = vi.fn();
    const onRepair = vi.fn();
    const r = report({ overall: "failed" });
    render(
      <ValidationPanel
        proposal={proposal()}
        report={r}
        validating={false}
        error={null}
        onValidate={vi.fn()}
        repairState={{ status: "idle", attempts: 0 } as any}
        onExplain={onExplain}
        onRepair={onRepair}
        maxAttempts={3}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Explain with AI" }));
    expect(onExplain).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Propose fix" }));
    expect(onRepair).toHaveBeenCalled();

    // Limit reached — clear DOM first to avoid duplicate buttons
    document.body.innerHTML = "";
    render(
      <ValidationPanel
        proposal={proposal()}
        report={r}
        validating={false}
        error={null}
        onValidate={vi.fn()}
        repairState={{ status: "error", attempts: 3, message: "limit" } as any}
        onExplain={vi.fn()}
        onRepair={vi.fn()}
        maxAttempts={3}
      />,
    );
    // Button should be disabled when limit reached
    const btn2 = screen.getByRole("button", { name: "Propose fix" }) as HTMLButtonElement;
    expect(btn2.disabled).toBe(true);
    expect(screen.getByText(/Repair limit reached/)).toBeTruthy();
  });

  it("disables Propose fix when validation passed", () => {
    const r = report({ overall: "passed" });
    render(
      <ValidationPanel
        proposal={proposal()}
        report={r}
        validating={false}
        error={null}
        onValidate={vi.fn()}
        repairState={null}
        onExplain={vi.fn()}
        onRepair={vi.fn()}
        maxAttempts={3}
      />,
    );
    const btn = screen.getByRole("button", { name: "Propose fix" }) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
  });

  it("shows repairing live state with collected preview", () => {
    render(
      <ValidationPanel
        proposal={proposal()}
        report={report()}
        validating={false}
        error={null}
        onValidate={vi.fn()}
        repairState={{ status: "repairing", collected: '{"version":1', attempts: 0 } as any}
        onExplain={vi.fn()}
        onRepair={vi.fn()}
        maxAttempts={3}
      />,
    );
    expect(screen.getByText(/Generating revised patch/)).toBeTruthy();
    expect(screen.getByText(/"version":1/)).toBeTruthy();
  });

  it("distinguishes original vs revised proposal", () => {
    const revised: PatchProposal = { ...proposal(), id: "patch-2", proposedContent: "fixed" };
    render(
      <ValidationPanel
        proposal={proposal()}
        report={report()}
        validating={false}
        error={null}
        onValidate={vi.fn()}
        repairState={{ status: "revised", proposal: revised, explanation: "fixed bracket", attempts: 1 } as any}
        onExplain={vi.fn()}
        onRepair={vi.fn()}
        maxAttempts={3}
        revisedProposal={revised}
      />,
    );
    expect(screen.getByText(/Revised proposal/)).toBeTruthy();
    expect(screen.getByText("not")).toBeTruthy();
    expect(screen.getByText(/been applied/)).toBeTruthy();
    expect(screen.getByText(/Original:/)).toBeTruthy();
    expect(screen.getByText(/fixed bracket/)).toBeTruthy();
  });

  it("shows error when repair fails schema validation", () => {
    render(
      <ValidationPanel
        proposal={proposal()}
        report={report()}
        validating={false}
        error={null}
        onValidate={vi.fn()}
        repairState={{ status: "error", message: "Malformed patch JSON", attempts: 1 } as any}
        onExplain={vi.fn()}
        onRepair={vi.fn()}
        maxAttempts={3}
      />,
    );
    expect(screen.getByText(/Malformed patch JSON/)).toBeTruthy();
  });
});
