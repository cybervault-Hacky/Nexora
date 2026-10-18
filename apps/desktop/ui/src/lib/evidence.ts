/**
 * Evidence-first — bounded representation for grounded answers.
 *
 * Phase 17: Do not duplicate the full indexes; reuse SymbolIndex + AnalysisGraph + RetrievalResult + ContextPlan
 * to derive a small, bounded EvidenceReport used for:
 *  - prompt grounding (formatEvidenceForPrompt)
 *  - UI transparency indicators (isPartial / isUnsupported / hasVerified …)
 *  - patch missing-source warnings (getPatchMissingEvidence)
 *
 * Bounded: entries ≤ MAX_EVIDENCE_ENTRIES (20), paths only, no file contents, no secrets in logs.
 * Sensitive files are excluded.
 */

import type { SymbolIndex } from "./symbolIndex";
import type { AnalysisGraph } from "./analysis";
import type { RetrievalResult } from "../features/workspace/retrieval";
import type { ContextPlan } from "../features/workspace/contextBudget";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------
export type EvidenceCategory = "verified" | "inferred" | "unavailable";
export type EvidenceSource = "indexed" | "retrieved" | "graph" | "selected" | "validation" | "none";

export interface EvidenceEntry {
  path: string;
  /** symbol name when this evidence is about a queried symbol */
  symbol?: string;
  kind?: string;
  line?: number;
  category: EvidenceCategory;
  source: EvidenceSource;
  /** short reason, never file contents */
  detail?: string;
}

export interface EvidenceLimitations {
  truncated: boolean;
  unsupported: number;
  parseErrors: number;
  graphTruncated: boolean;
  graphUnsupported: number;
  graphCycles: number;
  retrievalOmitted: number;
  retrievalWarnings: string[];
  budgetOmitted: number;
  budgetWarnings: string[];
  omittedPaths: string[];
  unsupportedLanguages: string[];
  indexUnavailable: boolean;
  graphUnavailable: boolean;
}

export interface EvidenceReport {
  entries: EvidenceEntry[];
  limitations: EvidenceLimitations;
  /** one-line human summary */
  summary: string;
  hasVerified: boolean;
  hasInferred: boolean;
  hasUnavailable: boolean;
  isPartial: boolean;
  isUnsupported: boolean;
  /** true when at least one verification step (validation/tests/build) was executed for current scope */
  verificationPerformed: boolean;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
export const MAX_EVIDENCE_ENTRIES = 20;
const MAX_LIMITATION_WARNINGS = 5;
const MAX_OMITTED_PATHS = 10;

function isSensitivePath(path: string): boolean {
  const base = path.split("/").pop() ?? "";
  if (base === ".env" || base === "credentials.json" || base.endsWith(".pem") || base.endsWith(".key")) return true;
  return path.includes("/.env");
}

function isSupportedPath(path: string): boolean {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return ["rs", "ts", "tsx", "js", "jsx", "py"].includes(ext);
}

function extOf(path: string): string {
  return path.split(".").pop()?.toLowerCase() ?? "";
}

// ---------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------
export interface BuildEvidenceArgs {
  symbolIndex: SymbolIndex | null;
  analysisGraph: AnalysisGraph | null;
  retrievalResult: RetrievalResult | null;
  contextPlan: ContextPlan | null;
  /** selected file paths (workspace selected) */
  selectedPaths?: string[];
  /** patch/validation context — when provided, report knows verification was attempted */
  verificationPerformed?: boolean;
}

export function buildEvidenceReport(args: BuildEvidenceArgs): EvidenceReport {
  const { symbolIndex, analysisGraph, retrievalResult, contextPlan, selectedPaths = [], verificationPerformed = false } = args;

  const truncated = !!symbolIndex?.truncated;
  const unsupported = symbolIndex?.unsupported ?? 0;
  const parseErrors = symbolIndex?.parseErrors ?? 0;
  const graphTruncated = !!analysisGraph?.truncated;
  const graphUnsupported = analysisGraph?.unsupported ?? 0;
  const graphCycles = analysisGraph?.cycles.length ?? 0;
  const retrievalOmitted = retrievalResult?.omitted.length ?? 0;
  const retrievalWarnings = (retrievalResult?.warnings ?? []).slice(0, MAX_LIMITATION_WARNINGS);
  const budgetOmitted = contextPlan ? contextPlan.filePlans.filter((p) => !p.included).length : 0;
  const budgetWarnings = (contextPlan?.warnings ?? []).slice(0, MAX_LIMITATION_WARNINGS);
  const omittedPathsCombined = [
    ...(retrievalResult?.omittedPaths ?? []),
    ...(contextPlan ? contextPlan.filePlans.filter((p) => !p.included).map((p) => p.path) : []),
  ].slice(0, MAX_OMITTED_PATHS);

  // Unsupported languages across selected + retrieved
  const unsupportedLangsSet = new Set<string>();
  const allPathsForLang = new Set<string>([
    ...selectedPaths,
    ...(retrievalResult?.includedPaths ?? []),
    ...(retrievalResult?.omittedPaths ?? []),
  ]);
  // also include from symbol index unsupported estimate — if any unsupported count >0 but we can't know ext, keep generic marker
  for (const p of allPathsForLang) {
    if (!isSupportedPath(p)) {
      const ext = extOf(p);
      if (ext) unsupportedLangsSet.add(ext);
      else unsupportedLangsSet.add("(no extension)");
    }
  }
  // If counts say unsupported but set empty, keep placeholder so UI can show
  if ((unsupported > 0 || graphUnsupported > 0) && unsupportedLangsSet.size === 0) {
    unsupportedLangsSet.add("unknown");
  }
  const unsupportedLanguages = Array.from(unsupportedLangsSet).sort().slice(0, 8);

  const limitations: EvidenceLimitations = {
    truncated,
    unsupported,
    parseErrors,
    graphTruncated,
    graphUnsupported,
    graphCycles,
    retrievalOmitted,
    retrievalWarnings,
    budgetOmitted,
    budgetWarnings,
    omittedPaths: omittedPathsCombined,
    unsupportedLanguages,
    indexUnavailable: symbolIndex == null,
    graphUnavailable: analysisGraph == null,
  };

  // Build entries (bounded)
  const entries: EvidenceEntry[] = [];

  // 1) Included retrieval entries — map to verified / inferred
  for (const inc of retrievalResult?.included ?? []) {
    if (isSensitivePath(inc.path)) continue;
    if (entries.length >= MAX_EVIDENCE_ENTRIES) break;
    const reason = inc.reason ?? "";
    const isVerifiedImport = reason.toLowerCase().includes("verified");
    const isSelected = reason === "selected by you";
    let category: EvidenceCategory;
    let source: EvidenceSource;
    if (isSelected || isVerifiedImport) {
      category = "verified";
      source = isSelected ? "selected" : "graph";
    } else if (inc.symbol) {
      category = "inferred";
      source = "indexed";
    } else {
      // graph-derived dependent without explicit symbol still inferred unless verified import already
      category = "inferred";
      source = "graph";
    }

    entries.push({
      path: inc.path,
      symbol: inc.symbol?.name,
      kind: inc.symbol?.kind,
      line: inc.symbol?.startLine,
      category,
      source,
      detail: reason.slice(0, 120),
    });
  }

  // 2) Omitted entries as unavailable (up to fill remaining budget)
  for (const om of retrievalResult?.omitted ?? []) {
    if (isSensitivePath(om.path)) continue;
    if (entries.length >= MAX_EVIDENCE_ENTRIES) break;
    // Don't duplicate a path already in entries
    if (entries.some((e) => e.path === om.path)) continue;
    entries.push({
      path: om.path,
      symbol: om.symbol?.name,
      kind: om.symbol?.kind,
      line: om.symbol?.startLine,
      category: "unavailable",
      source: "none",
      detail: (om.reason ?? "omitted (budget/limit)").slice(0, 120),
    });
  }

  // 3) Context-plan omitted files that weren't in retrieval omitted (e.g., filteredContextBlock dropped due to tokens)
  for (const plan of contextPlan?.filePlans.filter((p) => !p.included) ?? []) {
    if (isSensitivePath(plan.path)) continue;
    if (entries.length >= MAX_EVIDENCE_ENTRIES) break;
    if (entries.some((e) => e.path === plan.path)) continue;
    entries.push({
      path: plan.path,
      category: "unavailable",
      source: "none",
      detail: plan.omittedReason ?? "omitted (budget)",
    });
  }

  // 4) Selected paths not represented at all yet (edge: selected file larger than budget so not in retrieval included but still selected)
  for (const sel of selectedPaths) {
    if (isSensitivePath(sel)) continue;
    if (entries.length >= MAX_EVIDENCE_ENTRIES) break;
    if (entries.some((e) => e.path === sel)) continue;
    // If filePlans marks it included, we already have it; only add those that are missing (isolated selection)
    const plan = contextPlan?.filePlans.find((p) => p.path === sel);
    if (plan && !plan.included) continue; // already added as unavailable
    // If not in retrieval at all, note as unavailable so model knows selection not grounded for this query
    if (!retrievalResult?.includedPaths.includes(sel) && !retrievalResult?.omittedPaths.includes(sel)) {
      entries.push({
        path: sel,
        category: "unavailable",
        source: "selected",
        detail: "selected but not included in retrieved context for this query",
      });
    }
  }

  // Truncate if over (should not happen due to checks)
  const bounded = entries.slice(0, MAX_EVIDENCE_ENTRIES);

  const hasVerified = bounded.some((e) => e.category === "verified");
  const hasInferred = bounded.some((e) => e.category === "inferred");
  const hasUnavailable = bounded.some((e) => e.category === "unavailable");
  const isPartial =
    truncated ||
    graphTruncated ||
    retrievalOmitted > 0 ||
    budgetOmitted > 0 ||
    !!limitations.indexUnavailable ||
    !!limitations.graphUnavailable ||
    (limitations.retrievalWarnings.length > 0 && retrievalOmitted > 0) ||
    (limitations.budgetWarnings.length > 0 && budgetOmitted > 0);
  const isUnsupported = unsupported > 0 || graphUnsupported > 0 || unsupportedLanguages.length > 0 || parseErrors > 0;

  const summary = buildSummary(bounded, limitations, { hasVerified, hasInferred, hasUnavailable, isPartial, isUnsupported });

  return {
    entries: bounded,
    limitations,
    summary,
    hasVerified,
    hasInferred,
    hasUnavailable,
    isPartial,
    isUnsupported,
    verificationPerformed,
  };
}

function buildSummary(
  entries: EvidenceEntry[],
  limitations: EvidenceLimitations,
  flags: { hasVerified: boolean; hasInferred: boolean; hasUnavailable: boolean; isPartial: boolean; isUnsupported: boolean },
): string {
  const parts: string[] = [];
  if (entries.length === 0) {
    parts.push("No workspace files grounded for this query");
  } else {
    const verified = entries.filter((e) => e.category === "verified").length;
    const inferred = entries.filter((e) => e.category === "inferred").length;
    const unavailable = entries.filter((e) => e.category === "unavailable").length;
    const counts: string[] = [];
    if (verified) counts.push(`${verified} verified`);
    if (inferred) counts.push(`${inferred} inferred`);
    if (unavailable) counts.push(`${unavailable} unavailable`);
    parts.push(`Evidence: ${counts.join(", ")} — ${entries.length} file(s)`);
  }
  if (flags.isPartial) {
    const reasons: string[] = [];
    if (limitations.truncated) reasons.push("index truncated");
    if (limitations.graphTruncated) reasons.push("graph truncated");
    if (limitations.retrievalOmitted) reasons.push(`${limitations.retrievalOmitted} omitted (retrieval)`);
    if (limitations.budgetOmitted) reasons.push(`${limitations.budgetOmitted} omitted (budget)`);
    if (limitations.indexUnavailable) reasons.push("index unavailable");
    if (limitations.graphUnavailable) reasons.push("graph unavailable");
    if (reasons.length === 0) reasons.push("partial context");
    parts.push(`Partial context: ${reasons.join(", ")}`);
  } else if (entries.length > 0) {
    parts.push("Full context for this query");
  }
  if (flags.isUnsupported) {
    const bits: string[] = [];
    if (limitations.unsupported) bits.push(`${limitations.unsupported} file(s) not indexed`);
    if (limitations.graphUnsupported) bits.push(`${limitations.graphUnsupported} not graphed`);
    if (limitations.parseErrors) bits.push(`${limitations.parseErrors} parse error(s)`);
    if (limitations.unsupportedLanguages.length) bits.push(`unsupported: ${limitations.unsupportedLanguages.join(", ")}`);
    if (bits.length) parts.push(`Unsupported: ${bits.join("; ")}`);
  }
  if (limitations.graphCycles > 0) parts.push(`Cycles: ${limitations.graphCycles}`);
  return parts.join(" · ");
}

// ---------------------------------------------------------------------------
// Prompt formatting (do not include file contents, keep manifest bounded)
// ---------------------------------------------------------------------------
export function formatEvidenceForPrompt(report: EvidenceReport): string {
  const lines: string[] = [];
  lines.push("Evidence manifest (bounded, this query, no file contents):");

  // Show at most 8 entries in prompt to stay small; prefer verified first
  const sorted = [...report.entries].sort((a, b) => {
    const rank = (c: EvidenceCategory) => (c === "verified" ? 0 : c === "inferred" ? 1 : 2);
    return rank(a.category) - rank(b.category) || a.path.localeCompare(b.path);
  });
  const promptEntries = sorted.slice(0, 8);
  if (promptEntries.length === 0) {
    lines.push("- none — no workspace files were included as grounded context for this query");
  } else {
    for (const e of promptEntries) {
      const sym = e.symbol ? ` symbol \`${e.symbol}\`${e.line != null ? `:${e.line}` : ""} (${e.kind ?? "?"})` : "";
      const cat = e.category;
      lines.push(`- [${cat}] ${e.path}${sym} — ${e.detail ?? e.source}`);
    }
    if (report.entries.length > promptEntries.length) {
      lines.push(`- … ${report.entries.length - promptEntries.length} more (bounded)`);
    }
  }

  const lim = report.limitations;
  const limParts: string[] = [];
  if (lim.truncated) limParts.push("index truncated (500 files/5 MiB cap)");
  if (lim.graphTruncated) limParts.push("graph truncated");
  if (lim.unsupported) limParts.push(`${lim.unsupported} file(s) not indexed (unsupported)`);
  if (lim.graphUnsupported) limParts.push(`${lim.graphUnsupported} not graphed`);
  if (lim.parseErrors) limParts.push(`${lim.parseErrors} parse error(s)`);
  if (lim.retrievalOmitted) limParts.push(`${lim.retrievalOmitted} file(s) omitted (retrieval budget)`);
  if (lim.budgetOmitted) limParts.push(`${lim.budgetOmitted} file(s) omitted (token budget)`);
  if (lim.unsupportedLanguages.length) limParts.push(`unsupported langs: ${lim.unsupportedLanguages.join(", ")}`);
  if (lim.indexUnavailable) limParts.push("index unavailable");
  if (lim.graphUnavailable) limParts.push("graph unavailable");
  if (lim.graphCycles) limParts.push(`cycles=${lim.graphCycles}`);
  if (limParts.length) {
    lines.push(`Limitations: ${limParts.join("; ")}.`);
  } else {
    lines.push("Limitations: none for this query.");
  }
  if (report.isPartial) {
    lines.push("Note: context is partial — some relevant files may be missing for this query.");
  }

  // Reliability instructions — present as guidance, not rigid template
  lines.push(
    "Instructions: Use retrieved evidence for workspace factual claims; cite path:line when evidence supports it. Separate observations (cite path:line) from hypotheses (prefix ‘Hypothesis:’) and recommendations (prefix ‘Suggestion:’). Never invent files/symbols/test results/build/validation outcomes/line numbers. Static analysis does not prove runtime behavior — phrase import/dependency relationships as analysis results (verified vs unresolved) not runtime proof. If evidence is insufficient, respond ‘Not grounded in the provided files’ and list what you checked, then ask for clarification. Keep general coding explanations possible without workspace evidence when asked generally.",
  );
  lines.push(
    "Do not claim a test, build, or validation passed unless an actual result supports that claim; when no such result was provided, say ‘Verification not performed’. If this response was truncated or cancelled, note uncertainty rather than inventing.",
  );
  if (!report.verificationPerformed) {
    lines.push("Verification not performed for this query — do not invent test/build/validation results.");
  }
  if (lim.omittedPaths.length) {
    lines.push(`Omitted for this query (not inspected): ${lim.omittedPaths.join(", ")}`);
  }

  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// Patch missing-source helper
// ---------------------------------------------------------------------------
export function getPatchMissingEvidence(
  targetPath: string,
  report: EvidenceReport,
  retrievalResult: RetrievalResult | null,
  contextPlan: ContextPlan | null,
): { missing: string[]; warnings: string[] } {
  if (!targetPath) return { missing: [], warnings: [] };
  const missing: string[] = [];
  const warnings: string[] = [];

  const included = new Set(retrievalResult?.includedPaths ?? []);
  const planIncluded = contextPlan ? new Set(contextPlan.filePlans.filter((p) => p.included).map((p) => p.path)) : null;
  const isIncluded = included.has(targetPath);
  const isPlanIncluded = planIncluded ? planIncluded.has(targetPath) : true; // if no plan, ignore

  // If target not in retrieval included and not in plan included, it's missing evidence
  // Also if report marks it unavailable
  const reportUnavailable = report.entries.find((e) => e.path === targetPath && e.category === "unavailable");
  if (!isIncluded && reportUnavailable) {
    missing.push(targetPath);
    warnings.push(`Target file ${targetPath} was not included in retrieved context for this request.`);
  } else if (!isIncluded && !isPlanIncluded) {
    missing.push(targetPath);
    warnings.push(`Target file ${targetPath} could not be retrieved — patch will be based on the editor preview only.`);
  } else if (report.isPartial && (report.limitations.retrievalOmitted > 0 || report.limitations.budgetOmitted > 0)) {
    // Partial context but target was retrieved — still warn that dependencies may be missing
    const omitted = report.limitations.omittedPaths;
    if (omitted.length) {
      warnings.push(`Context is partial (${report.limitations.retrievalOmitted + report.limitations.budgetOmitted} file(s) omitted). Dependencies may be missing: ${omitted.slice(0, 3).join(", ")}${omitted.length > 3 ? " …" : ""}`);
    }
  }

  // Add graph unresolved note if target has unresolved imports (caller should provide graph; we use report flags)
  if (report.limitations.graphUnsupported > 0 || report.isUnsupported) {
    // Generic — detailed unresolved edges are in AnalysisPanel, not duplicated here
    if (missing.length === 0 && warnings.length === 0) {
      // only add if nothing else flagged but still partial
      if (report.isPartial) warnings.push("Some related files may be missing — graph shows unsupported or unresolved imports.");
    }
  }

  // Sensitive guard already handled elsewhere, but note if target is sensitive it should not be patched
  if (isSensitivePath(targetPath)) {
    warnings.push(`Target ${targetPath} is a sensitive file and cannot be patched.`);
    if (!missing.includes(targetPath)) missing.push(targetPath);
  }

  return { missing, warnings };
}

// Convenience: patch evidence available from included set
export function isPatchEvidenceAvailable(targetPath: string, retrievalResult: RetrievalResult | null, contextPlan: ContextPlan | null): boolean {
  if (!targetPath) return false;
  const included = retrievalResult?.includedPaths.includes(targetPath) ?? false;
  const planOk = contextPlan ? contextPlan.filePlans.find((p) => p.path === targetPath)?.included ?? true : true;
  return included && planOk;
}
