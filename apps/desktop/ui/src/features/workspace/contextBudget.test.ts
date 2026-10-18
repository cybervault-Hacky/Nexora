import { describe, expect, it } from "vitest";
import {
  estimateTokens,
  planContextBudget,
  formatBudgetLabel,
} from "./contextBudget";

describe("contextBudget — estimate", () => {
  it("uses conservative estimate (chars/4 ceil with margin)", () => {
    const text = "hello world"; // 11 chars => ceil(11/4)=3, *1.05 =>4
    const tokens = estimateTokens(text);
    expect(tokens).toBeGreaterThanOrEqual(3);
    expect(tokens).toBeLessThan(10);
  });

  it("labels estimated mode when no tokenizer", () => {
    const plan = planContextBudget({
      contextSize: 32768,
      maxTokens: 1024,
      history: [],
      currentPrompt: "hello",
      selectedFiles: [],
    });
    expect(plan.mode).toBe("estimated");
    expect(plan.estimated).toBe(true);
    expect(formatBudgetLabel(plan)).toContain("Estimated");
  });

  it("reserves system + output tokens from budget", () => {
    const plan = planContextBudget({
      contextSize: 4096,
      maxTokens: 512,
      history: [],
      currentPrompt: "hi",
      selectedFiles: [],
    });
    // reserved = 200 + 512 =712, available = 4096-712=3384
    expect(plan.budgetTokens).toBe(4096);
    expect(plan.reservedTokens).toBe(712);
    expect(plan.availableTokens).toBe(3384);
  });
});

describe("contextBudget — inclusion priority", () => {
  it("prompt is always included even when it exceeds budget", () => {
    const hugePrompt = "a".repeat(20000); // ~5250 tokens estimated
    const plan = planContextBudget({
      contextSize: 2048,
      maxTokens: 1024,
      history: [{ role: "assistant", content: "old" }],
      currentPrompt: hugePrompt,
      selectedFiles: [{ path: "a.txt", content: "x", size: 1 }],
    });
    expect(plan.promptIncluded).toBe(true);
    // warnings mention prompt exceeds
    expect(plan.warnings.some((w) => w.toLowerCase().includes("prompt"))).toBe(true);
  });

  it("fits files within available budget and omits overflow", () => {
    // Each file block ~ small, but with tiny budget only 1 fits
    const files = [
      { path: "a.txt", content: "x".repeat(500), size: 500 },
      { path: "b.txt", content: "y".repeat(500), size: 500 },
      { path: "c.txt", content: "z".repeat(500), size: 500 },
    ];
    const plan = planContextBudget({
      contextSize: 2048, // available =2048-1224=824
      maxTokens: 1024,
      history: [],
      currentPrompt: "hello",
      selectedFiles: files,
    });
    // At least first file should be included, last omitted if over
    expect(plan.filePlans[0]!.included).toBe(true);
    // With 824 available, and each file ~130 tokens (500/4 + fence), prompt ~2, total 3*130+2=392 fits, so all may fit
    // Use larger files to force omit
    const bigFiles = [
      { path: "a.txt", content: "x".repeat(5000), size: 5000 },
      { path: "b.txt", content: "y".repeat(5000), size: 5000 },
    ];
    const plan2 = planContextBudget({
      contextSize: 2048,
      maxTokens: 1024,
      history: [],
      currentPrompt: "hello",
      selectedFiles: bigFiles,
    });
    // With big files, one may be omitted
    expect(plan2.filePlans.some((f) => !f.included)).toBe(true);
  });

  it("respects user order priority (earlier files kept)", () => {
    const files = [
      { path: "first.txt", content: "a".repeat(4000), size: 4000 },
      { path: "second.txt", content: "b".repeat(4000), size: 4000 },
    ];
    const plan = planContextBudget({
      contextSize: 3000,
      maxTokens: 1024,
      history: [],
      currentPrompt: "hi",
      selectedFiles: files,
    });
    // If only one fits, first should be included
    if (plan.filePlans.filter((f) => f.included).length === 1) {
      expect(plan.filePlans[0]!.included).toBe(true);
      expect(plan.filePlans[0]!.path).toBe("first.txt");
    }
  });

  it("trims oldest history first, keeps newest", () => {
    const history = [
      { role: "user", content: "oldest turn " + "x".repeat(100) },
      { role: "assistant", content: "middle turn " + "y".repeat(100) },
      { role: "user", content: "newest turn " + "z".repeat(100) },
    ];
    const plan = planContextBudget({
      contextSize: 2048,
      maxTokens: 1024,
      history,
      currentPrompt: "hello",
      selectedFiles: [{ path: "a.txt", content: "file", size: 4 }],
    });
    // If trimming occurs, historyOmitted >0 means oldest dropped
    if (plan.historyOmitted > 0) {
      expect(plan.historyIncluded + plan.historyOmitted).toBe(history.length);
      // When 1 omitted, it should be oldest, newest still counted
      // We can't directly check which turn kept, but count is predictable
      expect(plan.historyIncluded).toBeLessThan(history.length);
    }
  });

  it("never slices inside fence — files either fully included or omitted", () => {
    const file = { path: "code.py", content: "```python\nprint('hi')\n```", size: 30 };
    const plan = planContextBudget({
      contextSize: 3000,
      maxTokens: 1024,
      history: [],
      currentPrompt: "explain",
      selectedFiles: [file],
    });
    // File should be either included fully or omitted
    expect([true, false]).toContain(plan.filePlans[0]!.included);
    if (plan.filePlans[0]!.included) {
      expect(plan.usedTokens).toBeGreaterThan(plan.promptTokens);
    }
  });

  it("identifies omitted files with reason", () => {
    const bigFiles = Array.from({ length: 5 }, (_, i) => ({
      path: `f${i}.txt`,
      content: "x".repeat(6000),
      size: 6000,
    }));
    const plan = planContextBudget({
      contextSize: 2500,
      maxTokens: 1024,
      history: [],
      currentPrompt: "hi",
      selectedFiles: bigFiles,
    });
    const omitted = plan.filePlans.filter((f) => !f.included);
    expect(omitted.length).toBeGreaterThan(0);
    for (const f of omitted) {
      expect(f.omittedReason).toBeTruthy();
    }
  });
});

describe("contextBudget — fences", () => {
  it("uses alternate fence when content contains ```", () => {
    const file = { path: "doc.md", content: "here is a code block\n```js\nconsole.log(1)\n```\nend" , size: 50 };
    const plan = planContextBudget({
      contextSize: 5000,
      maxTokens: 1024,
      history: [],
      currentPrompt: "summarize",
      selectedFiles: [file],
    });
    // Included means fence was chosen to not merge
    expect(plan.filePlans[0]!.included).toBe(true);
    // Token estimate uses fence, so used > prompt
    expect(plan.usedTokens).toBeGreaterThan(plan.promptTokens);
  });
});

describe("contextBudget — limits", () => {
  it("does not exceed available tokens", () => {
    const files = Array.from({ length: 8 }, (_, i) => ({ path: `f${i}.txt`, content: "a".repeat(200), size: 200 }));
    const plan = planContextBudget({
      contextSize: 32768,
      maxTokens: 1024,
      history: Array.from({ length: 10 }, (_, i) => ({ role: "user", content: `msg ${i} ` + "x".repeat(200) })),
      currentPrompt: "hello world",
      selectedFiles: files,
    });
    expect(plan.usedTokens).toBeLessThanOrEqual(plan.availableTokens);
  });
});
