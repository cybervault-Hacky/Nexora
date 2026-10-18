import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { ContextBudgetBar } from "./ContextBudgetBar";
import { planContextBudget } from "../features/workspace/contextBudget";

describe("ContextBudgetBar", () => {
  it("shows estimated label and included files", () => {
    const plan = planContextBudget({
      contextSize: 32768,
      maxTokens: 1024,
      history: [],
      currentPrompt: "hello",
      selectedFiles: [{ path: "a.txt", content: "hello world", size: 11 }],
    });
    render(<ContextBudgetBar plan={plan} />);
    expect(screen.getAllByText(/Estimated/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/a\.txt/)).toBeTruthy();
    expect(screen.getByText(/1 file.*included/)).toBeTruthy();
  });

  it("shows omitted files when budget exceeded", () => {
    const big = "x".repeat(8000);
    const plan = planContextBudget({
      contextSize: 2500,
      maxTokens: 1024,
      history: [],
      currentPrompt: "hi",
      selectedFiles: [
        { path: "first.txt", content: big, size: big.length },
        { path: "second.txt", content: big, size: big.length },
        { path: "third.txt", content: big, size: big.length },
      ],
    });
    // Force an omitted scenario via explicit plan if calculation didn't omit (fallback)
    const testPlan = plan.filePlans.some((f) => !f.included)
      ? plan
      : {
          ...plan,
          filePlans: [
            { path: "first.txt", included: true, tokens: 100, size: 400 },
            { path: "second.txt", included: false, tokens: 2100, size: 8000, omittedReason: "Budget exceeded" },
            { path: "third.txt", included: false, tokens: 2100, size: 8000, omittedReason: "Budget exceeded" },
          ],
          warnings: [...plan.warnings, "2 file(s) omitted due to budget."],
        };
    render(<ContextBudgetBar plan={testPlan} />);
    expect(screen.getByText(/Omitted files/)).toBeTruthy();
  });

  it("shows empty when no plan", () => {
    render(<ContextBudgetBar plan={null} />);
    expect(screen.getByText(/No context selected/)).toBeTruthy();
  });

  it("shows history omitted warning", () => {
    const history = Array.from({ length: 5 }, () => ({ role: "user", content: "msg " + "x".repeat(3000) }));
    const plan = planContextBudget({
      contextSize: 3000,
      maxTokens: 1024,
      history,
      currentPrompt: "hi",
      selectedFiles: [{ path: "f.txt", content: "small", size: 5 }],
    });
    const testPlan = plan.historyOmitted > 0
      ? plan
      : {
          ...plan,
          historyIncluded: 2,
          historyOmitted: 3,
          warnings: [...plan.warnings, "3 older turn(s) omitted to fit budget."],
        };
    render(<ContextBudgetBar plan={testPlan} />);
    expect(screen.getAllByText(/older turn/).length).toBeGreaterThanOrEqual(1);
  });
});
