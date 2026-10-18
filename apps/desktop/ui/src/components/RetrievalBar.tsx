import { Icon } from "./Icon";
import type { RetrievalResult } from "../features/workspace/retrieval";

interface Props {
  retrieval: RetrievalResult | null;
  onAdd: (path: string) => void;
  selectedPaths: Set<string>;
}

export function RetrievalBar({ retrieval, onAdd, selectedPaths }: Props) {
  if (!retrieval) return null;
  if (retrieval.included.length === 0 && retrieval.warnings.length === 0) return null;

  const suggestions = retrieval.included.filter((f) => !selectedPaths.has(f.path));
  const alreadyIncluded = retrieval.included.filter((f) => selectedPaths.has(f.path));

  // If only already selected and no warnings/omitted, don't show bar to avoid noise when draft empty
  if (suggestions.length === 0 && retrieval.warnings.length === 0 && retrieval.omitted.length === 0) {
    return null;
  }

  return (
    <div className="retrieval-bar" role="status" aria-label="Retrieved context">
      <div className="retrieval-bar__title">
        <Icon name="search" size={12} />
        <span>Retrieved for this prompt</span>
      </div>

      {alreadyIncluded.length > 0 ? (
        <p className="retrieval-bar__note">
          {alreadyIncluded.length} selected file{alreadyIncluded.length !== 1 ? "s" : ""} already in context: {alreadyIncluded.map((r) => r.path).join(", ")} — {alreadyIncluded[0]?.reason}
        </p>
      ) : null}

      {suggestions.length > 0 ? (
        <ul className="retrieval-bar__list" aria-label="Suggested files">
          {suggestions.map((file) => (
            <li key={file.path} className="retrieval-bar__item">
              <span className="retrieval-bar__path" title={file.path}>
                {file.path}
                {file.symbol ? ` — ${file.symbol.kind} \`${file.symbol.name}\`:${file.symbol.startLine}` : ""}
              </span>
              <span className="retrieval-bar__reason">{file.reason}</span>
              <button
                type="button"
                className="button button--ghost button--sm"
                onClick={() => onAdd(file.path)}
                title={`Add ${file.path} to context`}
                aria-label={`Add ${file.path} to context`}
              >
                Add
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="retrieval-bar__note">No additional relevant files found — using selected files only. Eligible files only, budget respected, no invented symbols.</p>
      )}

      {retrieval.omitted.length > 0 ? (
        <details className="retrieval-bar__omitted">
          <summary>Omitted {retrieval.omitted.length} (budget)</summary>
          <ul className="retrieval-bar__list">
            {retrieval.omitted.map((o) => (
              <li key={o.path} className="retrieval-bar__item retrieval-bar__item--omitted" title={o.reason}>
                <span className="retrieval-bar__path">{o.path}</span>
                <span className="retrieval-bar__reason">{o.reason}</span>
              </li>
            ))}
          </ul>
        </details>
      ) : null}

      {retrieval.warnings.length > 0 ? (
        <ul className="retrieval-bar__warnings" aria-label="Retrieval warnings">
          {retrieval.warnings.map((w, i) => (
            <li key={i}>⚠ {w}</li>
          ))}
        </ul>
      ) : null}

      <p className="retrieval-bar__note">
        Deterministic retrieval: selected files first, then symbol/path signals. No duplicate snippets, no whole-workspace dump. Indexed facts only.
      </p>
    </div>
  );
}
