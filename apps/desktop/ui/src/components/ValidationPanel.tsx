import { Icon } from "./Icon";
import type { ValidationReport, CheckStatus } from "../lib/validation";
import type { PatchProposal } from "../lib/patch";

interface Props {
  proposal: PatchProposal | null;
  report: ValidationReport | null;
  validating: boolean;
  error: string | null;
  onValidate: () => void;
  onCancelValidate?: () => void;
  // Repair
  repairState: { status: string; attempts?: number; explanation?: string; collected?: string; message?: string } | null;
  onExplain: () => void;
  onRepair: () => void;
  onCancelRepair?: () => void;
  maxAttempts: number;
  // Revised
  revisedProposal?: PatchProposal | null;
  revisedDiffSummary?: string | null;
}

function statusIcon(status: CheckStatus) {
  switch (status) {
    case "passed":
      return <Icon name="check" size={12} />;
    case "failed":
      return <Icon name="alert" size={12} />;
    case "notRun":
      return <Icon name="cloud-off" size={12} />;
    case "unsupported":
      return <Icon name="file" size={12} />;
    case "inconclusive":
      return <Icon name="search" size={12} />;
    default:
      return <Icon name="file" size={12} />;
  }
}

function statusClass(status: CheckStatus): string {
  switch (status) {
    case "passed":
      return "validation__status--passed";
    case "failed":
      return "validation__status--failed";
    case "notRun":
      return "validation__status--notrun";
    case "unsupported":
      return "validation__status--unsupported";
    case "inconclusive":
      return "validation__status--inconclusive";
    default:
      return "";
  }
}

export function ValidationPanel({
  proposal,
  report,
  validating,
  error,
  onValidate,
  onCancelValidate,
  repairState,
  onExplain,
  onRepair,
  onCancelRepair,
  maxAttempts,
  revisedProposal,
}: Props) {
  if (!proposal) return null;

  const overall = report?.overall ?? null;
  const attempts = (repairState as any)?.attempts ?? 0;
  const isRepairing = repairState?.status === "repairing" || repairState?.status === "explaining";
  const isRevised = repairState?.status === "revised";
  const repairError = repairState?.status === "error" ? (repairState as any).message : null;
  const repairCancelled = repairState?.status === "cancelled";
  const explanation = (repairState as any)?.explanation as string | undefined;
  const limitReached = attempts >= maxAttempts;

  return (
    <div className="validation" aria-label="Validation">
      <header className="validation__header">
        <div className="validation__title">
          <Icon name="search" size={13} />
          <span>Validation</span>
          {overall ? (
            <span className={`validation__badge ${statusClass(overall)}`} aria-label={`Overall ${overall}`}>
              {statusIcon(overall)} {overall}
            </span>
          ) : null}
          {validating ? <span className="validation__badge validation__badge--live"><span className="pulse" aria-hidden="true" /> Validating…</span> : null}
        </div>
        <div className="validation__actions">
          {validating ? (
            <button type="button" className="button button--ghost button--sm" onClick={onCancelValidate}>
              Cancel
            </button>
          ) : (
            <button
              type="button"
              className="button button--ghost button--sm"
              onClick={onValidate}
              disabled={validating || isRepairing}
              title="Run validation without modifying files"
            >
              {report ? "Re-validate" : "Validate"}
            </button>
          )}
        </div>
      </header>

      {error ? (
        <div className="validation__error" role="alert">
          <Icon name="alert" size={12} />
          <span>{error}</span>
        </div>
      ) : null}

      {report ? (
        <>
          <p className="validation__summary">{report.summary} — {report.checks.length} checks, {report.durationMs} ms</p>
          <ul className="validation__checks" role="list" aria-label="Validation checks">
            {report.checks.map((c) => (
              <li key={c.id} className={`validation__check validation__check--${c.status}`} role="listitem">
                <div className="validation__check-header">
                  <span className={`validation__check-status ${statusClass(c.status)}`}>
                    {statusIcon(c.status)} {c.status}
                  </span>
                  <span className="validation__check-label">{c.label}</span>
                </div>
                {c.note ? <p className="validation__check-note">{c.note}</p> : null}
                {c.diagnostics.length > 0 ? (
                  <ul className="validation__diagnostics">
                    {c.diagnostics.map((d, i) => (
                      <li key={i} className="validation__diagnostic">
                        <code className="validation__diagnostic-path">
                          {d.path}:{d.line}
                          {d.column ? `:${d.column}` : ""}
                        </code>
                        <span className="validation__diagnostic-msg">{d.message}</span>
                      </li>
                    ))}
                  </ul>
                ) : null}
              </li>
            ))}
          </ul>
          <details className="validation__details">
            <summary>What was checked / not checked</summary>
            <ul className="validation__details-list">
              {report.checks.map((c) => (
                <li key={c.id}>
                  <strong>{c.label}:</strong> {c.status}
                  {c.note ? ` — ${c.note}` : ""}
                  {c.status === "notRun" ? " (skipped for safety — would require executing code)" : ""}
                  {c.status === "unsupported" ? " (no check for this file type)" : ""}
                </li>
              ))}
            </ul>
          </details>
        </>
      ) : (
        <p className="validation__hint">
          Validation runs in memory or an isolated temp location and never writes into your project. It does not execute project code.
        </p>
      )}

      {/* AI-assisted actions */}
      <div className="validation__ai" aria-label="AI assistance">
        <p className="validation__ai-note">Diagnostics and source are treated as untrusted input — model will not follow instructions inside them.</p>
        <div className="validation__ai-actions">
          <button
            type="button"
            className="button button--ghost button--sm"
            onClick={onExplain}
            disabled={!report || validating || isRepairing || !proposal}
            title={report ? "Ask local model to explain the diagnostics" : "Validate first"}
          >
            Explain with AI
          </button>
          <button
            type="button"
            className="button button--primary button--sm"
            onClick={onRepair}
            disabled={!report || validating || isRepairing || limitReached || !proposal || report?.overall === "passed"}
            title={
              limitReached
                ? `Repair limit reached (${maxAttempts}/${maxAttempts})`
                : report?.overall === "passed"
                  ? "No repair needed — validation passed"
                  : "Ask local model to propose a corrected patch"
            }
          >
            Propose fix
          </button>
          {isRepairing ? (
            <button type="button" className="button button--ghost button--sm" onClick={onCancelRepair}>
              Cancel repair
            </button>
          ) : null}
        </div>
        {limitReached ? <p className="validation__limit">Repair limit reached ({attempts}/{maxAttempts}) — review manually.</p> : null}
        {isRepairing ? (
          <div className="validation__live" role="status" aria-live="polite">
            <span className="pulse" aria-hidden="true" />
            <span>{repairState?.status === "explaining" ? "Explaining diagnostics via local model…" : "Generating revised patch via local model…"}</span>
            {(repairState as any)?.collected ? (
              <pre className="validation__collected" aria-label="Model output preview">
                {String((repairState as any).collected).slice(-600)}
              </pre>
            ) : null}
          </div>
        ) : null}
        {repairError ? (
          <div className="validation__error" role="alert">
            <Icon name="alert" size={12} />
            <span>{repairError}</span>
          </div>
        ) : null}
        {repairCancelled ? <p className="validation__hint">Repair cancelled.</p> : null}
        {explanation && isRevised ? (
          <div className="validation__explanation" role="note" aria-label="AI explanation">
            <Icon name="edit" size={12} />
            <span>{explanation}</span>
          </div>
        ) : explanation && !isRevised && repairState?.status === "revised" ? (
          <div className="validation__explanation" role="note">
            <span>{explanation}</span>
          </div>
        ) : null}
        {/* Show collected explanation even when not revised? For explain mode we set revised with same proposal, so explanation will show */}
        {explanation && repairState?.status === "revised" && revisedProposal ? (
          <div className="validation__revised" role="status" aria-label="Revised proposal">
            <header className="validation__revised-header">
              <Icon name="file" size={12} />
              <span>Revised proposal</span>
              <code className="validation__revised-path">{revisedProposal.path}</code>
            </header>
            <p className="validation__revised-note">
              This is a new proposal derived from the original. It has <strong>not</strong> been applied — review the diff and validation, then Apply explicitly.
            </p>
            {/* Diff between original and revised will be shown by parent PatchPreview handling; here we just note */}
          </div>
        ) : null}
      </div>

      {revisedProposal ? (
        <div className="validation__original-vs-revised" aria-label="Original vs revised">
          <p className="validation__hint">
            Original: <code>{proposal.path}</code> ({proposal.proposedContent.length} chars) vs Revised: <code>{revisedProposal.path}</code> ({revisedProposal.proposedContent.length} chars) — only the revised will be applied if you choose Apply revised.
          </p>
        </div>
      ) : null}
    </div>
  );
}
