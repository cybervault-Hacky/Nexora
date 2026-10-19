import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "./Icon";
import { getSymbolIndex, refreshSymbolIndex, searchSymbols, type Symbol, type SymbolIndex } from "../lib/symbolIndex";

interface Props {
  workspacePath: string | null;
  onPreview: (path: string) => void;
  externalIndex?: SymbolIndex | null;
  externalLoading?: boolean;
  externalError?: string | null;
  onExternalRefresh?: () => void;
  onIndexChange?: (idx: SymbolIndex | null) => void;
}

export function CodeIntelligencePanel({ workspacePath, onPreview, externalIndex, externalLoading, externalError, onExternalRefresh, onIndexChange }: Props) {
  const isControlled = externalIndex !== undefined;
  const [internalIndex, setInternalIndex] = useState<SymbolIndex | null>(null);
  const [internalLoading, setInternalLoading] = useState(false);
  const [internalError, setInternalError] = useState<string | null>(null);
  const index = isControlled ? externalIndex ?? null : internalIndex;
  const loading = isControlled ? (externalLoading ?? false) : internalLoading;
  const error = isControlled ? (externalError ?? null) : internalError;
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Symbol[]>([]);
  const [kindFilter, setKindFilter] = useState<"all" | "functions" | "types" | "constants">("all");
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searchedQuery, setSearchedQuery] = useState("");
  const searchSequence = useRef(0);

  const filteredResults = useMemo(() => {
    if (kindFilter === "all") return results;
    if (kindFilter === "functions") {
      return results.filter((s) => s.kind === "function" || s.kind === "method");
    }
    if (kindFilter === "types") {
      return results.filter(
        (s) =>
          s.kind === "class" ||
          s.kind === "interface" ||
          s.kind === "type" ||
          s.kind === "struct" ||
          s.kind === "enum" ||
          s.kind === "trait",
      );
    }
    if (kindFilter === "constants") {
      return results.filter((s) => s.kind === "constant");
    }
    return results;
  }, [results, kindFilter]);

  const load = useCallback(async (refresh = false) => {
    if (isControlled) {
      if (onExternalRefresh && refresh) onExternalRefresh();
      // In controlled mode, parent drives fetching; but on first open, parent already fetched via useEffect in App
      // So we trigger externalRefresh for both initial and refresh if parent provided
      if (onExternalRefresh && !refresh && externalIndex === undefined) onExternalRefresh();
      return;
    }
    if (!workspacePath) {
      setInternalIndex(null);
      setInternalError(null);
      onIndexChange?.(null);
      return;
    }
    setInternalLoading(true);
    setInternalError(null);
    try {
      const idx = refresh ? await refreshSymbolIndex(workspacePath) : await getSymbolIndex(workspacePath);
      setInternalIndex(idx);
      onIndexChange?.(idx);
    } catch (cause) {
      const msg = (cause as { message?: string })?.message ?? String(cause);
      setInternalError(msg);
      setInternalIndex(null);
      onIndexChange?.(null);
    } finally {
      setInternalLoading(false);
    }
  }, [workspacePath, isControlled, onExternalRefresh, externalIndex]);

  useEffect(() => {
    searchSequence.current += 1;
    setQuery("");
    setResults([]);
    setSearching(false);
    setSearchError(null);
    setSearchedQuery("");
    if (isControlled) {
      // Parent's effect already loads; just ensure query reset
      return;
    }
    if (workspacePath) {
      void load(false);
    } else {
      setInternalIndex(null);
      setInternalError(null);
    }
  }, [workspacePath, load, isControlled]);

  const handleSearch = useCallback(async () => {
    const normalizedQuery = query.trim();
    if (!workspacePath || !normalizedQuery) {
      searchSequence.current += 1;
      setResults([]);
      setSearchError(null);
      setSearching(false);
      setSearchedQuery("");
      return;
    }

    const requestId = ++searchSequence.current;
    setSearchedQuery(normalizedQuery);
    setResults([]);
    setSearching(true);
    setSearchError(null);
    try {
      const res = await searchSymbols(workspacePath, normalizedQuery, 20);
      if (requestId === searchSequence.current) {
        setResults(res);
      }
    } catch (cause) {
      if (requestId === searchSequence.current) {
        const msg = (cause as { message?: string })?.message ?? String(cause);
        setSearchError(msg);
        setResults([]);
      }
    } finally {
      if (requestId === searchSequence.current) {
        setSearching(false);
      }
    }
  }, [workspacePath, query]);

  const handleQueryChange = (value: string) => {
    searchSequence.current += 1;
    setQuery(value);
    setResults([]);
    setSearching(false);
    setSearchError(null);
    setSearchedQuery("");
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      void handleSearch();
    }
  };

  if (!workspacePath) {
    return (
      <div className="code-intel" role="status" aria-label="Code intelligence">
        <p className="code-intel__hint">Open a project to see code intelligence.</p>
      </div>
    );
  }

  // Compute file type counts from index if available, else from index is null
  const fileTypeCounts = (() => {
    if (!index) return null;
    const counts = new Map<string, number>();
    for (const sym of index.symbols) {
      const ext = sym.path.split(".").pop()?.toLowerCase() ?? "unknown";
      counts.set(ext, (counts.get(ext) ?? 0) + 1);
    }
    // Also count files via fileCount
    return counts;
  })();

  return (
    <div className="code-intel" aria-label="Code intelligence">
      <header className="code-intel__header">
        <div className="code-intel__title">
          <Icon name="file" size={14} />
          <span>Code Intelligence</span>
        </div>
        <button
          type="button"
          className="button button--ghost button--sm"
          onClick={() => void load(true)}
          disabled={loading}
          title="Rebuild index"
          aria-label="Refresh index"
        >
          <Icon name="refresh" size={12} />
          {loading ? "Indexing…" : "Refresh"}
        </button>
      </header>

      {loading ? (
        <p className="code-intel__status" role="status">Indexing… Scanning eligible files (max 500 files, 5 MiB)…</p>
      ) : error ? (
        <div className="code-intel__error" role="alert">
          <Icon name="alert" size={12} />
          <span>{error}</span>
          <button type="button" className="button button--ghost button--sm" onClick={() => void load(true)}>
            Retry
          </button>
        </div>
      ) : index ? (
        <div className="code-intel__overview" role="status" aria-label="Project overview">
          <div className="code-intel__stats">
            <span>{index.fileCount} files indexed</span>
            <span>{index.symbols.length} symbols</span>
            {index.truncated ? <span className="code-intel__warn">truncated (limit hit)</span> : null}
            {index.unsupported > 0 ? <span>{index.unsupported} unsupported</span> : null}
            {index.parseErrors > 0 ? <span>{index.parseErrors} parse errors</span> : null}
          </div>
          {fileTypeCounts && fileTypeCounts.size > 0 ? (
            <ul className="code-intel__filetypes" aria-label="File types">
              {Array.from(fileTypeCounts.entries()).map(([ext, count]) => (
                <li key={ext} className="code-intel__filetype">
                  <span>.{ext}</span>
                  <span>{count}</span>
                </li>
              ))}
            </ul>
          ) : null}
          <p className="code-intel__meta" title={index.root}>
            Indexed {new Date(index.generatedAt).toLocaleTimeString()}
          </p>
          <p className="code-intel__hint">
            Rust, TypeScript, JavaScript, and Python via regex; others are unsupported, with no symbols invented.
          </p>
        </div>
      ) : (
        <p className="code-intel__hint">No index yet.</p>
      )}

      <div className="code-intel__search" role="search" aria-label="Search symbols">
        <label htmlFor="symbol-search" className="code-intel__label">
          Search symbols
        </label>
        <div className="code-intel__search-row" aria-busy={searching}>
          <div className="search-field code-intel__field">
            <Icon className="search-field__icon" name="search" size={15} />
            <input
              id="symbol-search"
              className="code-intel__input search-field__input"
              type="text"
              value={query}
              onChange={(e) => handleQueryChange(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Name a symbol…"
              aria-label="Search symbols by name"
              autoComplete="off"
              spellCheck={false}
              maxLength={200}
            />
            {query ? (
              <button
                type="button"
                className="search-field__clear"
                aria-label="Clear symbol search"
                title="Clear search"
                onClick={() => handleQueryChange("")}
              >
                <Icon name="x" size={13} />
              </button>
            ) : null}
          </div>
          <button
            type="button"
            className="button button--primary button--sm code-intel__submit"
            onClick={() => void handleSearch()}
            disabled={searching || !query.trim()}
            aria-label={searching ? "Searching symbols" : "Search symbols"}
            title={searching ? "Searching symbols" : "Search symbols"}
          >
            {searching ? "Searching…" : <><Icon name="search" size={13} /> Search</>}
          </button>
        </div>
        {searchError ? (
          <p className="code-intel__error" role="alert">
            {searchError}
          </p>
        ) : null}
        {results.length > 0 ? (
          <>
            <div className="code-intel__filters" role="radiogroup" aria-label="Filter symbols by kind">
              <button
                type="button"
                className={`button button--ghost button--sm ${kindFilter === "all" ? "button--selected" : ""}`}
                onClick={() => setKindFilter("all")}
                aria-checked={kindFilter === "all"}
                role="radio"
              >
                All ({results.length})
              </button>
              <button
                type="button"
                className={`button button--ghost button--sm ${kindFilter === "functions" ? "button--selected" : ""}`}
                onClick={() => setKindFilter("functions")}
                aria-checked={kindFilter === "functions"}
                role="radio"
              >
                Functions
              </button>
              <button
                type="button"
                className={`button button--ghost button--sm ${kindFilter === "types" ? "button--selected" : ""}`}
                onClick={() => setKindFilter("types")}
                aria-checked={kindFilter === "types"}
                role="radio"
              >
                Types & Classes
              </button>
              <button
                type="button"
                className={`button button--ghost button--sm ${kindFilter === "constants" ? "button--selected" : ""}`}
                onClick={() => setKindFilter("constants")}
                aria-checked={kindFilter === "constants"}
                role="radio"
              >
                Constants
              </button>
            </div>
            {filteredResults.length > 0 ? (
              <ul className="code-intel__results" role="list" aria-label="Search results" aria-live="polite">
                {filteredResults.map((sym) => (
                  <li key={`${sym.path}:${sym.name}:${sym.startLine}`} className="code-intel__result" role="listitem">
                    <button
                      type="button"
                      className="code-intel__result-button"
                      onClick={() => onPreview(sym.path)}
                      title={`${sym.path}:${sym.startLine}`}
                      aria-label={`Open ${sym.path}:${sym.startLine}`}
                    >
                      <span className="code-intel__result-name">{sym.name}</span>
                      <span className="code-intel__result-kind">{sym.kind}</span>
                      <span className="code-intel__result-path">
                        {sym.path}:{sym.startLine}
                      </span>
                    </button>
                    {sym.detail ? <span className="code-intel__result-detail">{sym.detail}</span> : null}
                  </li>
                ))}
              </ul>
            ) : (
              <p className="code-intel__hint" role="status">
                No {kindFilter} symbols in search results for “{searchedQuery}”.
              </p>
            )}
          </>
        ) : searchedQuery === query.trim() && searchedQuery && !searching && !searchError ? (
          <p className="code-intel__hint" role="status">
            No symbols found for “{searchedQuery}”. Try another name or refresh the index.
            Unsupported languages return no symbols.
          </p>
        ) : null}
      </div>
    </div>
  );
}
