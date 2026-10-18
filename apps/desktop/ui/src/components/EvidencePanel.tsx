import { Icon } from "./Icon";
import type { EvidenceReport } from "../lib/evidence";

interface Props {
  report: EvidenceReport | null;
  isStreaming?: boolean;
  wasCancelled?: boolean;
}

export function EvidencePanel({ report, isStreaming, wasCancelled }: Props) {
  if (!report) return null;

  const showAvailable = report.hasVerified;
  const showPartial = report.isPartial;
  const showInferred = report.hasInferred;
  const showUnsupported = report.isUnsupported;
  const showVerificationNotPerformed = !report.verificationPerformed;
  const showStreamingIncomplete = !!isStreaming || !!wasCancelled;

  // Only render when there's something to show and not empty idle with no draft
  // If report is empty and no limits, still show summary as neutral
  const anyIndicator = showAvailable || showPartial || showInferred || showUnsupported || showVerificationNotPerformed || showStreamingIncomplete;
  if (!anyIndicator && report.entries.length === 0) return null;

  return (
    <div className="evidence" role="status" aria-label="Evidence transparency">
      <div className="evidence__header">
        <span className="evidence__title">
          <Icon name="shield" size={12} />
          Evidence
        </span>
        <span className="evidence__summary" title={report.summary}>
          {report.summary}
        </span>
      </div>

      <div className="evidence__chips">
        {showAvailable ? (
          <span className="evidence__chip evidence__chip--verified" title="Workspace factual claims are supported by retrieved files">
            <Icon name="check" size={11} />
            Evidence available
          </span>
        ) : null}
        {showPartial ? (
          <span className="evidence__chip evidence__chip--partial" title={`Partial context: ${report.limitations.truncated ? "index truncated. " : ""}${report.limitations.retrievalOmitted ? `${report.limitations.retrievalOmitted} omitted (retrieval). ` : ""}${report.limitations.budgetOmitted ? `${report.limitations.budgetOmitted} omitted (budget). ` : ""}${report.limitations.omittedPaths.slice(0,3).join(", ") || "Some files not inspected"}`}>
            <Icon name="alert" size={11} />
            Partial context
          </span>
        ) : null}
        {showInferred ? (
          <span className="evidence__chip evidence__chip--inferred" title="Some relationships are inferred from symbol matches or static analysis, not verified imports">
            <Icon name="branch" size={11} />
            Inferred relationship
          </span>
        ) : null}
        {showUnsupported ? (
          <span className="evidence__chip evidence__chip--unsupported" title={`Unsupported: ${report.limitations.unsupported} not indexed, ${report.limitations.graphUnsupported} not graphed${report.limitations.unsupportedLanguages.length ? ` — ${report.limitations.unsupportedLanguages.join(", ")}` : ""}${report.limitations.parseErrors ? `, ${report.limitations.parseErrors} parse errors` : ""}`}>
            <Icon name="file" size={11} />
            Unsupported analysis
          </span>
        ) : null}
        {showVerificationNotPerformed ? (
          <span className="evidence__chip evidence__chip--verification" title="No test, build or validation has been executed for this query — results are not to be invented">
            <Icon name="circle" size={11} />
            Verification not performed
          </span>
        ) : null}
        {showStreamingIncomplete ? (
          <span className="evidence__chip evidence__chip--streaming" title="Response may be incomplete due to streaming interruption">
            <Icon name="alert" size={11} />
            Response incomplete
          </span>
        ) : null}
      </div>

      {report.entries.length > 0 ? (
        <details className="evidence__details">
          <summary>Evidence manifest ({report.entries.length} — bounded)</summary>
          <ul className="evidence__list">
            {report.entries.slice(0, 8).map((e) => (
              <li key={`${e.path}:${e.symbol ?? ""}`} className="evidence__item">
                <span className={`evidence__badge evidence__badge--${e.category}`}>{e.category}</span>
                <span className="evidence__path" title={e.path}>
                  {e.path}
                  {e.symbol ? ` — ${e.kind ?? ""} \`${e.symbol}\`${e.line != null ? `:${e.line}` : ""}` : ""}
                </span>
                <span className="evidence__detail">{e.detail}</span>
              </li>
            ))}
          </ul>
          {report.entries.length > 8 ? <p className="evidence__note">… {report.entries.length - 8} more (bounded at {report.entries.length})</p> : null}
          {(report.limitations.retrievalWarnings.length > 0 || report.limitations.budgetWarnings.length > 0) ? (
            <ul className="evidence__warnings">
              {report.limitations.retrievalWarnings.slice(0, 3).map((w, i) => (
                <li key={`r-${i}`} className="evidence__warning">Retrieval: {w}</li>
              ))}
              {report.limitations.budgetWarnings.slice(0, 3).map((w, i) => (
                <li key={`b-${i}`} className="evidence__warning">Budget: {w}</li>
              ))}
            </ul>
          ) : null}
          {report.limitations.omittedPaths.length > 0 ? (
            <p className="evidence__note evidence__note--omitted">Not inspected: {report.limitations.omittedPaths.slice(0,5).join(", ")}{report.limitations.omittedPaths.length > 5 ? " …" : ""}</p>
          ) : null}
        </details>
      ) : (
        <p className="evidence__hint">No workspace files grounded for this query — general explanations are possible, but workspace claims will be marked “Not grounded in the provided files”.</p>
      )}

      <p className="evidence__footnote">Verified paths/lines only — never invent. Inferred relationships labelled — suggestions stay suggestions until verified. Static import analysis does not prove runtime.</p>
    </div>
  );
}
