import { useMemo } from "react";
// @ts-ignore — diff ships without bundled types in this setup; runtime import is correct
import { diffLines } from "diff";
import { Icon } from "./Icon";
import type { PatchProposal, PatchValidation } from "../lib/patch";

interface Props {
  proposal: PatchProposal | null;
  validation: PatchValidation | null;
  loading?: boolean;
  error?: string | null;
  onCancel: () => void;
  onApply: () => void;
  applying?: boolean;
  applied?: boolean;
}

export function PatchPreview({ proposal, validation, loading, error, onCancel, onApply, applying, applied }: Props) {
  const hunks = useMemo(() => {
    if (!proposal) return [];
    const changes = diffLines(proposal.originalContent, proposal.proposedContent);
    const result: Array<{ type: "added" | "removed" | "unchanged"; text: string }> = [];
    for (const part of changes) {
      const lines = part.value.split("\n");
      // diffLines includes trailing newline as empty last line
      for (let i = 0; i < lines.length; i++) {
        const line = lines[i]!;
        if (i === lines.length - 1 && line === "") continue;
        if (part.added) result.push({ type: "added", text: line });
        else if (part.removed) result.push({ type: "removed", text: line });
        else result.push({ type: "unchanged", text: line });
      }
    }
    return result;
  }, [proposal]);

  if (loading) {
    return (
      <div className="patch-preview" role="status" aria-label="Patch preview">
        <p className="patch-preview__status">Validating proposal…</p>
      </div>
    );
  }

  if (error) {
    const isStale = error.toLowerCase().includes("stale") || error.toLowerCase().includes("changed since");
    return (
      <div className="patch-preview patch-preview--error" role="alert" aria-label="Patch error">
        <header className="patch-preview__header">
          <Icon name="alert" size={14} />
          <span>{isStale ? "Stale proposal" : "Invalid proposal"}</span>
          <button type="button" className="patch-preview__close" onClick={onCancel} aria-label="Dismiss">
            <Icon name="x" size={12} />
          </button>
        </header>
        <p className="patch-preview__message">{error}</p>
        {isStale ? <p className="patch-preview__note">The file was modified after this proposal was created. Reload the file and create a new proposal.</p> : null}
        <div className="patch-preview__actions">
          <button type="button" className="button button--ghost" onClick={onCancel}>
            Dismiss
          </button>
        </div>
      </div>
    );
  }

  if (applied) {
    return (
      <div className="patch-preview patch-preview--applied" role="status" aria-label="Patch applied">
        <header className="patch-preview__header">
          <Icon name="check" size={14} />
          <span>Patch applied</span>
          <button type="button" className="patch-preview__close" onClick={onCancel} aria-label="Dismiss">
            <Icon name="x" size={12} />
          </button>
        </header>
        <p className="patch-preview__message">File {proposal?.path} was updated. Only the intended file was changed.</p>
      </div>
    );
  }

  if (!proposal || !validation) {
    return (
      <div className="patch-preview patch-preview--empty" aria-label="Patch preview">
        <p className="patch-preview__hint">No proposal to preview. Select a file, edit, and preview the change. Reviewing a diff does not modify the file.</p>
      </div>
    );
  }

  if (!validation.ok) {
    return (
      <div className="patch-preview patch-preview--invalid" role="alert">
        <header className="patch-preview__header">
          <Icon name="alert" size={14} />
          <span>Invalid proposal</span>
          <button type="button" className="patch-preview__close" onClick={onCancel} aria-label="Cancel">
            <Icon name="x" size={12} />
          </button>
        </header>
        <ul className="patch-preview__errors">
          {validation.errors.map((e, i) => (
            <li key={i}>{e}</li>
          ))}
        </ul>
        {validation.warnings.length > 0 ? (
          <ul className="patch-preview__warnings">
            {validation.warnings.map((w, i) => (
              <li key={i}>{w}</li>
            ))}
          </ul>
        ) : null}
        <div className="patch-preview__actions">
          <button type="button" className="button button--ghost" onClick={onCancel}>
            Cancel
          </button>
        </div>
        <p className="patch-preview__note">Reviewing a diff does not itself modify the file.</p>
      </div>
    );
  }

  const stale = validation.warnings.some((w) => w.toLowerCase().includes("stale"));
  const summary = validation.diffSummary;

  return (
    <div className="patch-preview" aria-label="Patch preview">
      <header className="patch-preview__header">
        <div className="patch-preview__title">
          <Icon name="file" size={13} />
          <span title={proposal.path}>{proposal.path}</span>
          <span className="patch-preview__summary" title={`+${summary.added} -${summary.removed} ~${summary.unchanged}`}>
            +{summary.added} / -{summary.removed} / {summary.unchanged} unchanged
          </span>
        </div>
        <button type="button" className="patch-preview__close" onClick={onCancel} aria-label="Cancel">
          <Icon name="x" size={13} />
        </button>
      </header>

      {stale ? (
        <div className="patch-preview__stale" role="alert">
          <Icon name="alert" size={12} />
          <span>File has changed since this proposal was created. Apply is disabled — reload and recreate.</span>
        </div>
      ) : null}

      {validation.warnings.length > 0 && !stale ? (
        <ul className="patch-preview__warnings" role="note">
          {validation.warnings.map((w, i) => (
            <li key={i}>{w}</li>
          ))}
        </ul>
      ) : null}

      <div className="patch-preview__diff" role="region" aria-label="Diff">
        <pre className="patch-preview__code" tabIndex={0}>
          {hunks.map((h, idx) => (
            <span key={idx} className={`patch-line patch-line--${h.type}`}>
              <span className="patch-line__gutter" aria-hidden="true">
                {h.type === "added" ? "+" : h.type === "removed" ? "-" : " "}
              </span>
              <code>{h.text || " "}</code>
            </span>
          ))}
          {hunks.length === 0 ? <span className="patch-line patch-line--unchanged"><code>(no changes)</code></span> : null}
        </pre>
      </div>

      <p className="patch-preview__note">Reviewing a diff does not modify the file. Only the shown file will be changed on Apply.</p>

      <div className="patch-preview__actions">
        <button type="button" className="button button--ghost" onClick={onCancel} disabled={!!applying}>
          Cancel
        </button>
        <button
          type="button"
          className="button button--primary"
          onClick={onApply}
          disabled={!!applying || stale || !validation.ok}
          aria-label="Apply patch to file"
          title={stale ? "Stale — reload first" : "Apply this change to the file"}
        >
          {applying ? "Applying…" : "Apply change"}
        </button>
      </div>
    </div>
  );
}
