/**
 * Context budget planner for Phase 12.
 *
 * Uses the configured `contextSize` (tokens) where available, reserves
 * capacity for system overhead + model output, then fits the current prompt
 * and selected files within the remaining budget. History is trimmed
 * predictably (oldest first). File boundaries and fences are never sliced.
 *
 * Token counting:
 * - If a real tokenizer for the active model is available, use it and mark
 *   `mode: "measured"`.
 * - Otherwise use a documented conservative estimate: `ceil(chars / 4)`.
 *   This is the industry-average for English/code (range 3–5 chars/token).
 *   We label it `Estimated` and never present it as exact.
 * - Conservative = may over-estimate for plain English but can still
 *   under-estimate for dense code; we stay within the char limits
 *   (`MAX_CONTEXT_*`, `MAX_PREVIEW_BYTES`) as a second guard.
 */

export interface ContextFilePlan {
  path: string;
  included: boolean;
  tokens: number; // estimated or measured
  size: number;
  omittedReason?: string;
}

export interface ContextPlan {
  mode: "estimated" | "measured";
  budgetTokens: number;
  reservedTokens: number;
  availableTokens: number;
  usedTokens: number;
  remainingTokens: number;
  estimated: boolean;
  warnings: string[];
  filePlans: ContextFilePlan[];
  historyIncluded: number;
  historyOmitted: number;
  promptTokens: number;
  promptIncluded: boolean;
}

export const SYSTEM_OVERHEAD_TOKENS = 200;
export const DEFAULT_BUDGET_TOKENS = 32768;
export const MIN_BUDGET_TOKENS = 2048;

/**
 * Conservative token estimate. ~4 chars per token for English/code.
 * We round up and add a 5% safety margin so the estimate is slightly
 * pessimistic (over-estimates tokens) to stay inside real limits.
 */
export function estimateTokens(text: string): number {
  if (!text) return 0;
  // chars → tokens: 1 token ≈ 4 chars. Use ceil + 5% margin, min 1.
  const base = Math.ceil(text.length / 4);
  const withMargin = Math.ceil(base * 1.05);
  return Math.max(1, withMargin);
}

/**
 * Measured token count if a tokenizer is available.
 * Currently no tokenizer is bundled, so we fall back to estimate.
 * This hook is where a future `tiktoken` or `llama.cpp` tokenizer would plug in.
 */
export function measureTokens(text: string, tokenizer?: { encode: (s: string) => number[] }): { tokens: number; mode: "estimated" | "measured" } {
  if (tokenizer && typeof tokenizer.encode === "function") {
    try {
      const tokens = tokenizer.encode(text).length;
      return { tokens, mode: "measured" };
    } catch {
      // fall through to estimate
    }
  }
  return { tokens: estimateTokens(text), mode: "estimated" };
}

function fenceFor(content: string): string {
  return content.includes("```") ? "````" : "```";
}

function inferLang(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  const map: Record<string, string> = {
    rs: "rust",
    ts: "typescript",
    tsx: "tsx",
    js: "javascript",
    jsx: "jsx",
    py: "python",
    json: "json",
    md: "markdown",
    toml: "toml",
    yaml: "yaml",
    yml: "yaml",
    css: "css",
    html: "html",
    sh: "bash",
    go: "go",
    java: "java",
  };
  return map[ext] ?? "";
}

function fileBlockTokens(file: { path: string; content: string }, tokenizer?: { encode: (s: string) => number[] }): number {
  const lang = inferLang(file.path);
  const fence = fenceFor(file.content);
  const block = `\nFile: ${file.path}\n${fence}${lang}\n${file.content}\n${fence}\n`;
  return measureTokens(block, tokenizer).tokens;
}

export interface PlanArgs {
  contextSize: number; // tokens, from Settings / AppStatus
  maxTokens: number; // reserved for model output, e.g. 1024
  systemOverhead?: number; // default 200
  history: Array<{ role: string; content: string }>;
  currentPrompt: string;
  selectedFiles: Array<{ path: string; content: string; size: number }>;
  tokenizer?: { encode: (s: string) => number[] };
}

/**
 * Plans which selected files and how much history fit.
 *
 * Priority (never dropped):
 * 1) currentPrompt (always included, error if alone exceeds budget)
 * 2) selectedFiles in user-chosen order (earlier = higher priority)
 * 3) recent history (newest turns kept, oldest trimmed first)
 *
 * If budget is exceeded, we omit entire files (never slice inside a fence)
 * and drop oldest history turns first, reporting omitted files/history.
 */
export function planContextBudget(args: PlanArgs): ContextPlan {
  const { history, currentPrompt, selectedFiles, tokenizer } = args;
  const budgetTokens = Math.max(MIN_BUDGET_TOKENS, args.contextSize || DEFAULT_BUDGET_TOKENS);
  const reserved = (args.systemOverhead ?? SYSTEM_OVERHEAD_TOKENS) + (args.maxTokens ?? 1024);
  const available = Math.max(0, budgetTokens - reserved);

  const promptMeasure = measureTokens(currentPrompt, tokenizer);
  const promptTokens = promptMeasure.tokens;
  const mode: "estimated" | "measured" = promptMeasure.mode; // if any measured, we would propagate; for now estimated

  const warnings: string[] = [];
  if (promptTokens > available) {
    warnings.push("Current prompt alone exceeds the available budget; history and some files must be omitted.");
  }

  // Build file plans in order, fitting within available after prompt + history overhead.
  // We first calculate history tokens if we kept all, then trim.

  // Estimate history tokens newest-first, but we need to know how many we can keep.
  const historyTokensPerTurn = history.map((t) => measureTokens(`${t.role}: ${t.content}`, tokenizer).tokens);
  const totalHistoryTokens = historyTokensPerTurn.reduce((a, b) => a + b, 0);

  // File tokens
  const fileTokens = selectedFiles.map((f) => fileBlockTokens(f, tokenizer));

  // Greedy inclusion: prompt first, then files in order, then history newest-first.
  let used = promptTokens;
  const filePlans: ContextFilePlan[] = [];
  let fileUsed = 0;

  for (let i = 0; i < selectedFiles.length; i++) {
    const f = selectedFiles[i]!;
    const tokens = fileTokens[i]!;
    if (used + tokens <= available || selectedFiles.length === 0) {
      // Actually we need to check after including files, history may still exceed.
      // We will include file if it fits assuming minimal history (0) — then trim history later if needed.
      // So first pass: include files that fit with prompt alone.
      if (used + tokens <= available) {
        filePlans.push({ path: f.path, included: true, tokens, size: f.size });
        used += tokens;
        fileUsed += tokens;
      } else {
        filePlans.push({ path: f.path, included: false, tokens, size: f.size, omittedReason: "Budget exceeded" });
      }
    } else {
      filePlans.push({ path: f.path, included: false, tokens, size: f.size, omittedReason: "Budget exceeded" });
    }
  }

  // Now fit history: keep newest turns, drop oldest until within budget
  let historyIncluded = history.length;
  let historyUsed = totalHistoryTokens;
  let omitted = 0;

  // If used + historyUsed > available, drop oldest turns
  while (historyIncluded > 0 && used + historyUsed > available) {
    // Drop oldest (index 0)
    const droppedTokens = historyTokensPerTurn[history.length - historyIncluded]!;
    historyUsed -= droppedTokens;
    historyIncluded -= 1;
    omitted += 1;
  }

  // If still over, we need to drop files from last to first (lowest priority)
  // This keeps prompt always.
  let fileIdx = filePlans.length - 1;
  while (fileIdx >= 0 && used + historyUsed > available) {
    const plan = filePlans[fileIdx];
    if (plan && plan.included) {
      used -= plan.tokens;
      fileUsed -= plan.tokens;
      plan.included = false;
      plan.omittedReason = "Budget exceeded — files trimmed after history";
      historyIncluded = history.length; // try to re-include history after dropping file? loop will recompute?
      // Recompute historyIncluded after file drop: we had dropped oldest, but after freeing file tokens we may be able to keep more history
      // For simplicity, after dropping a file, reset history to max that fits with new used
      historyIncluded = history.length;
      historyUsed = totalHistoryTokens;
      omitted = 0;
      while (historyIncluded > 0 && used + historyUsed > available) {
        const droppedTokens = historyTokensPerTurn[history.length - historyIncluded]!;
        historyUsed -= droppedTokens;
        historyIncluded -= 1;
        omitted += 1;
      }
    }
    fileIdx -= 1;
  }

  const finalUsed = promptTokens + fileUsed + historyUsed;
  const finalHistoryOmitted = history.length - historyIncluded;
  const omittedFiles = filePlans.filter((p) => !p.included);

  if (omittedFiles.length > 0) {
    warnings.push(`${omittedFiles.length} file(s) omitted due to budget.`);
  }
  if (finalHistoryOmitted > 0) {
    warnings.push(`${finalHistoryOmitted} older turn(s) omitted to fit budget.`);
  }

  return {
    mode,
    budgetTokens,
    reservedTokens: reserved,
    availableTokens: available,
    usedTokens: finalUsed,
    remainingTokens: Math.max(0, available - finalUsed),
    estimated: mode === "estimated",
    warnings,
    filePlans,
    historyIncluded,
    historyOmitted: finalHistoryOmitted,
    promptTokens,
    promptIncluded: promptTokens <= available || promptTokens > 0, // always true, prompt never dropped
  };
}

export function formatBudgetLabel(plan: ContextPlan): string {
  const type = plan.estimated ? "Estimated" : "Measured";
  return `${type}: ${plan.usedTokens} / ${plan.availableTokens} tokens (budget ${plan.budgetTokens}, reserved ${plan.reservedTokens})`;
}
