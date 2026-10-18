import type { Symbol, SymbolIndex } from "../../lib/symbolIndex";

export interface RetrievedFile {
  path: string;
  reason: string;
  symbol?: Symbol;
}

export interface RetrievalResult {
  included: RetrievedFile[];
  omitted: RetrievedFile[];
  warnings: string[];
  includedPaths: string[];
  omittedPaths: string[];
}

const MAX_RETRIEVED_FILES = 8;
const MAX_RETRIEVED_BYTES = 100 * 1024;

function isSensitivePath(path: string): boolean {
  const base = path.split("/").pop() ?? "";
  if (base === ".env" || base === "credentials.json" || base.endsWith(".pem") || base.endsWith(".key")) return true;
  return path.includes("/.env");
}

function extractQuerySymbols(query: string): string[] {
  if (!query || !query.trim()) return [];
  // Split on non-alphanumeric/underscore, keep tokens length>2, dedup, lower not needed for case-insensitive but keep original
  const tokens = query
    .split(/[^A-Za-z0-9_]+/)
    .map((t) => t.trim())
    .filter((t) => t.length > 2 && t.length < 50)
    .filter((t, idx, arr) => arr.indexOf(t) === idx)
    .slice(0, 10);
  return tokens;
}

// Deterministic ranking: exact name then prefix then substring then path
export function rankSymbols(symbols: Symbol[], queryTokens: string[]): Symbol[] {
  if (queryTokens.length === 0) return [];
  const lowerTokens = queryTokens.map((t) => t.toLowerCase());
  const scored: Array<{ sym: Symbol; score: number }> = [];
  for (const sym of symbols) {
    const nameLower = sym.name.toLowerCase();
    const pathLower = sym.path.toLowerCase();
    let best = Infinity;
    for (const tok of lowerTokens) {
      let score = Infinity;
      if (nameLower === tok) score = 0;
      else if (nameLower.startsWith(tok)) score = 1;
      else if (nameLower.includes(tok)) score = 2;
      else if (pathLower.includes(tok)) score = 3;
      else continue;
      if (score < best) best = score;
    }
    if (best !== Infinity) {
      scored.push({ sym, score: best });
    }
  }
  // Sort by score then path then name for determinism
  scored.sort((a, b) => {
    if (a.score !== b.score) return a.score - b.score;
    if (a.sym.path !== b.sym.path) return a.sym.path.localeCompare(b.sym.path);
    return a.sym.name.localeCompare(b.sym.name);
  });
  return scored.map((s) => s.sym);
}

export interface RetrieveArgs {
  query: string;
  selectedFiles: Array<{ path: string; content: string; size: number }>;
  symbolIndex: SymbolIndex | null;
  availableFiles?: Array<{ path: string; size: number }>; // from inventory, to know what exists but not selected
  maxFiles?: number;
  maxBytes?: number;
}

// Deterministic, bounded retrieval
export function retrieveRelevantFiles(args: RetrieveArgs): RetrievalResult {
  const { query, selectedFiles, symbolIndex } = args;
  const maxFiles = args.maxFiles ?? MAX_RETRIEVED_FILES;
  const maxBytes = args.maxBytes ?? MAX_RETRIEVED_BYTES;

  const warnings: string[] = [];
  const included: RetrievedFile[] = [];
  const omitted: RetrievedFile[] = [];
  const seen = new Set<string>();

  // 1. Selected files — highest priority, never omitted unless over budget (but we respect budget)
  // They are already filtered via is_sensitive, but double-check
  const selectedFiltered = selectedFiles.filter((f) => !isSensitivePath(f.path));
  // Track bytes for budget
  let totalBytes = 0;
  for (const f of selectedFiltered) {
    if (seen.has(f.path)) continue;
    // Check budget before adding
    const size = f.size ?? f.content.length;
    if (included.length >= maxFiles || totalBytes + size > maxBytes) {
      omitted.push({ path: f.path, reason: "Budget exceeded — selected file omitted (unexpected)" });
      continue;
    }
    // De-duplicate check already
    seen.add(f.path);
    included.push({ path: f.path, reason: "selected by you" });
    totalBytes += size;
  }

  if (symbolIndex && query.trim()) {
    const tokens = extractQuerySymbols(query);
    if (tokens.length === 0) {
      // No symbols to search, fallback to selected only
      return {
        included,
        omitted,
        warnings: included.length === 0 ? ["No relevant symbols found — using selected files only."] : warnings,
        includedPaths: included.map((r) => r.path),
        omittedPaths: omitted.map((r) => r.path),
      };
    }

    const ranked = rankSymbols(symbolIndex.symbols, tokens);
    // Map symbols to files, dedup by path, keep best symbol per file
    const fileToSymbol = new Map<string, Symbol>();
    for (const sym of ranked) {
      if (isSensitivePath(sym.path)) continue;
      if (seen.has(sym.path)) continue; // already selected, skip
      if (!fileToSymbol.has(sym.path)) {
        fileToSymbol.set(sym.path, sym);
      }
    }

    // Add retrieved files in ranked order
    for (const [path, sym] of fileToSymbol) {
      if (included.length >= maxFiles) {
        omitted.push({ path, reason: `defines symbol \`${sym.name}\` — omitted (budget)`, symbol: sym });
        continue;
      }
      // We don't have content size here; estimate via symbol detail or use average
      // For now, assume 2000 bytes per retrieved file (conservative), but if we have selectedFiles already, we know sizes
      // We should check if path is in availableFiles to get real size, else estimate
      const estimatedSize = 4000; // conservative for budget check (we will re-check with real content when building context)
      if (totalBytes + estimatedSize > maxBytes) {
        omitted.push({ path, reason: `defines symbol \`${sym.name}\` — omitted (budget)`, symbol: sym });
        continue;
      }
      seen.add(path);
      included.push({ path, reason: `defines symbol \`${sym.name}\` (${sym.kind})`, symbol: sym });
      totalBytes += estimatedSize;
    }

    if (included.length === selectedFiltered.length && fileToSymbol.size === 0) {
      warnings.push("No relevant symbols found — using selected files only.");
    }
  } else if (!symbolIndex) {
    if (query.trim() && included.length === 0) {
      warnings.push("Index unavailable — answer based on selected files only.");
    } else if (!query.trim() && included.length === 0) {
      warnings.push("No files selected — index unavailable.");
    }
  }

  // Prevent duplicate snippets — already deduped by path
  // Ensure we don't exceed limits after all adds
  // If we added estimated sizes, actual sizes may differ; but we keep deterministic

  // If still no included and no omitted and query present, note fallback
  if (included.length === 0 && omitted.length === 0) {
    if (query.trim()) {
      warnings.push("No relevant files found for query.");
    } else if (symbolIndex) {
      warnings.push("No relevant symbols found \u2014 using selected files only.");
    }
  }

  return {
    included,
    omitted,
    warnings,
    includedPaths: included.map((r) => r.path),
    omittedPaths: omitted.map((r) => r.path),
  };
}

export interface SmartRetrieveArgs extends RetrieveArgs {
  graph?: { nodes: Array<{ path: string; imports: Array<{ to: string; verified: boolean }>; importedBy: string[] }>; edges: Array<{ from: string; to: string; verified: boolean }> } | null;
}

/**
 * Phase 16: dependency-aware retrieval.
 * Extends retrieveRelevantFiles by prioritizing files related via the dependency graph:
 * - files imported by selected files (dependencies) — verified edges first
 * - files that import selected files (dependents)
 * - for symbol queries, files that import the defining file of the top-ranked symbol
 * All are bounded by maxFiles/maxBytes, sensitive excluded, never duplicate, never whole-repo.
 */
export function retrieveRelevantFilesSmart(args: SmartRetrieveArgs): RetrievalResult {
  // Start with baseline
  const base = retrieveRelevantFiles(args);
  if (!args.graph || !args.graph.nodes.length) return base;

  const graph = args.graph;
  const seen = new Set<string>(base.included.map((r) => r.path));
  const selectedPaths = args.selectedFiles.map((f) => f.path);
  // Build quick lookup: path -> node
  const nodeMap = new Map<string, { imports: Array<{ to: string; verified: boolean }>; importedBy: string[] }>();
  for (const n of graph.nodes) {
    nodeMap.set(n.path, n);
  }

  // Collect candidate related paths with reason
  const candidates: Array<{ path: string; reason: string; priority: number }> = [];

  // 1) For each selected file, add its verified dependencies and its dependents
  for (const sel of selectedPaths) {
    const node = nodeMap.get(sel);
    if (!node) continue;
    for (const imp of node.imports) {
      if (!imp.verified) continue;
      if (seen.has(imp.to)) continue;
      if (isSensitivePath(imp.to)) continue;
      candidates.push({ path: imp.to, reason: `dependency of ${sel} (verified import)`, priority: 1 });
    }
    for (const depBy of node.importedBy) {
      if (seen.has(depBy)) continue;
      if (isSensitivePath(depBy)) continue;
      candidates.push({ path: depBy, reason: `dependent of ${sel} (imports it)`, priority: 1 });
    }
  }

  // 2) For symbol-based inclusions already in base, also pull their dependents (impact)
  // Find top symbol defining file(s) and add files that import them
  const symbolDefPaths = new Set<string>();
  for (const inc of base.included) {
    if (inc.symbol) symbolDefPaths.add(inc.symbol.path);
  }
  for (const defPath of symbolDefPaths) {
    const node = nodeMap.get(defPath);
    if (!node) continue;
    for (const depBy of node.importedBy) {
      if (seen.has(depBy)) continue;
      if (isSensitivePath(depBy)) continue;
      // Already in candidates? dedup later
      candidates.push({ path: depBy, reason: `imports ${defPath} which defines queried symbol`, priority: 2 });
    }
  }

  // Deduplicate candidates preserving priority order (verified first)
  candidates.sort((a, b) => a.priority - b.priority || a.path.localeCompare(b.path));
  const unique = new Map<string, { path: string; reason: string }>();
  for (const c of candidates) {
    if (!unique.has(c.path) && !seen.has(c.path)) {
      unique.set(c.path, { path: c.path, reason: c.reason });
    }
  }

  // Merge into base respecting budget (use same maxFiles/maxBytes logic as base)
  const maxFiles = args.maxFiles ?? MAX_RETRIEVED_FILES;
  const maxBytes = args.maxBytes ?? MAX_RETRIEVED_BYTES;
  let totalBytes = base.included.reduce((sum, r) => {
    const sel = args.selectedFiles.find((f) => f.path === r.path);
    return sum + (sel ? sel.size : 4000);
  }, 0);

  const mergedIncluded = [...base.included];
  const mergedOmitted = [...base.omitted];
  const mergedWarnings = [...base.warnings];

  for (const [path, info] of unique) {
    if (mergedIncluded.length >= maxFiles) {
      mergedOmitted.push({ path, reason: `${info.reason} — omitted (budget)` });
      continue;
    }
    const estimatedSize = 4000;
    if (totalBytes + estimatedSize > maxBytes) {
      mergedOmitted.push({ path, reason: `${info.reason} — omitted (budget)` });
      continue;
    }
    mergedIncluded.push({ path, reason: info.reason });
    totalBytes += estimatedSize;
    seen.add(path);
  }

  // If we added graph-informed files, add a note about verified vs inferred
  if (mergedIncluded.length > base.included.length) {
    mergedWarnings.push(
      `Graph-aware: added ${mergedIncluded.length - base.included.length} related file(s) via verified imports`,
    );
  }

  return {
    included: mergedIncluded,
    omitted: mergedOmitted,
    warnings: mergedWarnings,
    includedPaths: mergedIncluded.map((r) => r.path),
    omittedPaths: mergedOmitted.map((r) => r.path),
  };
}

export function buildGroundedContext(
  retrieved: RetrievalResult,
  fileContents: Map<string, string>,
  languageHint: (path: string) => string,
  fenceFor: (content: string) => string,
): string | null {
  if (retrieved.included.length === 0) return null;

  let block = "Relevant files (grounded — indexed facts, verified paths/lines):\n";
  for (const file of retrieved.included) {
    const content = fileContents.get(file.path);
    if (!content) {
      block += `\nFile: ${file.path} (not inspected — ${file.reason})\n`;
      continue;
    }
    const lang = languageHint(file.path);
    const fence = fenceFor(content);
    const symbolNote = file.symbol ? ` — ${file.symbol.kind} \`${file.symbol.name}\`:${file.symbol.startLine}` : "";
    block += `\nFile: ${file.path}${symbolNote} — ${file.reason}\n${fence}${lang}\n${content}\n${fence}\n`;
  }
  if (retrieved.omitted.length > 0) {
    block += `\nOmitted files (budget): ${retrieved.omitted.map((o) => o.path).join(", ")}\n`;
  }
  block += "\nUse the files above as grounded context. Cite file paths and lines when answering. Distinguish indexed facts from generated interpretations. If you could not inspect a file, say so.\n";
  return block;
}

// Helper to check if file is supported (for UI)
export function isSupportedFile(path: string): boolean {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return ["rs", "ts", "tsx", "js", "jsx", "py"].includes(ext);
}
