import { useRef } from "react";
import { useExport } from "../features/export/useExport";
import type { ExportPlan } from "../lib/export";
import { Icon } from "./Icon";

interface ExportPanelProps {
  workspacePath: string | null;
  selectedPaths: ReadonlySet<string>;
  previewPath: string | null;
}

function gib(bytes: number): string {
  return (bytes / 1024 ** 3).toFixed(2);
}

function plural(count: number): string {
  return count === 1 ? "file" : "files";
}

function withheld(plan: ExportPlan): string[] {
  const parts: string[] = [];

  if (plan.excludedSensitive > 0) {
    parts.push(`${plan.excludedSensitive} secret ${plural(plan.excludedSensitive)} withheld`);
  }

  if (plan.excludedSymlink > 0) {
    parts.push(`${plan.excludedSymlink} symbolic link${plan.excludedSymlink === 1 ? "" : "s"} skipped`);
  }

  if (plan.excludedUnsafe > 0) {
    parts.push(`${plan.excludedUnsafe} unsafe ${plural(plan.excludedUnsafe)} skipped`);
  }

  return parts;
}

/**
 * Download Mode.
 *
 * An export workflow, not a generator: it lists the files the workspace
 * scanner would include, lets the user review them, and writes a copy where the
 * user chooses. Nothing is saved on mount or without an explicit action.
 */
export function ExportPanel({ workspacePath, selectedPaths, previewPath }: ExportPanelProps) {
  const controller = useExport({ workspacePath, selectedPaths, previewPath });
  const destinationRef = useRef<HTMLInputElement | null>(null);

  const {
    scope,
    setScope,
    status,
    plan,
    destination,
    setDestination,
    progress,
    outcome,
    error,
    conflict,
    note,
    planNote,
    cancelling,
    selection,
    canExport,
    chooseDestination,
    start,
    cancel,
    replace,
    dismissNote,
  } = controller;

  const saving = status === "saving";
  const isZip = scope !== "file";
  const hidden = plan ? withheld(plan) : [];

  return (
    <section className="export" aria-label="Download Mode">
      <header className="export__header">
        <div className="export__title">
          <Icon name="download" size={14} />
          <div>
            <p className="eyebrow">Download Mode</p>
            <h2 className="export__heading">Export project files</h2>
          </div>
        </div>
        {plan && isZip ? (
          <span className="export__badge">
            {plan.files.length} {plural(plan.files.length)} · {gib(plan.totalBytes)} GiB
          </span>
        ) : null}
      </header>

      <p className="export__copy">
        Generated files stay in your project folder. Download Mode writes{" "}
        <strong>copies</strong> to a location you choose; it never changes or deletes the project.
      </p>

      <div className="export__scopes" role="group" aria-label="Export scope">
        <button
          type="button"
          className="button button--ghost button--sm"
          aria-pressed={scope === "project"}
          onClick={() => setScope("project")}
          disabled={saving}
        >
          Whole project
        </button>
        <button
          type="button"
          className="button button--ghost button--sm"
          aria-pressed={scope === "selection"}
          onClick={() => setScope("selection")}
          disabled={saving || selection.length === 0}
          title={
            selection.length === 0
              ? "Select files in the project explorer first"
              : "Export only the selected files"
          }
        >
          Selected files ({selection.length})
        </button>
        <button
          type="button"
          className="button button--ghost button--sm"
          aria-pressed={scope === "file"}
          onClick={() => setScope("file")}
          disabled={saving || !previewPath}
          title={previewPath ? "Export the file currently previewed" : "Preview a file first"}
        >
          Single file
        </button>
      </div>

      <div className="export__review" role="status" aria-live="polite">
        {status === "preparing" ? (
          <span>Preparing the file list…</span>
        ) : scope === "file" ? (
          <span className="export__mono">{previewPath ?? "Preview a file to export it."}</span>
        ) : plan ? (
          <>
            <span>
              {plan.files.length} {plural(plan.files.length)} · {gib(plan.totalBytes)} GiB will be
              written.
            </span>
            <ul className="export__list">
              {plan.files.slice(0, 8).map((file) => (
                <li key={file.path} className="export__list-item">
                  <span className="export__mono">{file.path}</span>
                  <span className="export__size">{file.size.toLocaleString("en-US")} B</span>
                </li>
              ))}
              {plan.files.length > 8 ? (
                <li className="export__list-more">and {plan.files.length - 8} more</li>
              ) : null}
            </ul>
            <span className="export__exclusions">
              {hidden.length > 0
                ? `Withheld: ${hidden.join(", ")}.`
                : "No secrets or symbolic links were found in the export set."}{" "}
              Build output, dependencies, caches and .gitignore matches are excluded by the
              workspace scanner.
            </span>
          </>
        ) : planNote ? (
          <span>{planNote}</span>
        ) : null}
      </div>

      <div className="field export__destination">
        <label className="field__label" htmlFor="export-destination">
          {isZip ? "Save archive to" : "Save file to"}
        </label>
        <div className="field__row">
          <input
            id="export-destination"
            ref={destinationRef}
            className="field__input"
            value={destination}
            onChange={(event) => setDestination(event.target.value)}
            placeholder={isZip ? "/home/you/project.zip" : "/home/you/exported-file"}
            spellCheck={false}
            disabled={saving}
            aria-describedby="export-destination-hint"
          />
          <button
            type="button"
            className="button button--ghost button--sm"
            onClick={chooseDestination}
            disabled={saving}
            title="Choose a destination with the native dialog when it is installed"
          >
            Choose…
          </button>
        </div>
        <p className="field__hint" id="export-destination-hint">
          Absolute path. An existing file is never replaced without a confirmation.
        </p>
      </div>

      {conflict ? (
        <div className="export__conflict" role="alert">
          <p>{conflict}</p>
          <div className="export__actions">
            <button type="button" className="button button--primary button--sm" onClick={replace}>
              Replace existing file
            </button>
            <button
              type="button"
              className="button button--ghost button--sm"
              onClick={() => {
                setDestination(destination);
                destinationRef.current?.focus();
              }}
            >
              Choose another name
            </button>
          </div>
        </div>
      ) : null}

      {saving ? (
        <p className="export__progress">
          {progress
            ? `Writing ${progress.done} of ${progress.total} · ${progress.current || "finishing"}`
            : "Writing the export…"}
        </p>
      ) : null}

      {error ? (
        <p className="export__error" role="alert">
          <Icon name="alert" size={13} />
          {error.message}
        </p>
      ) : null}

      {status === "completed" && outcome ? (
        <p className="export__success" role="status">
          Saved {outcome.files} {plural(outcome.files)} ({gib(outcome.bytes)} GiB) to{" "}
          <span className="export__mono">{outcome.path}</span>.
          {outcome.replaced ? " The previous file was replaced." : ""}
        </p>
      ) : null}

      {status === "cancelled" ? (
        <p className="export__note" role="status">
          Export cancelled. Nothing was written.
        </p>
      ) : null}

      {note && status !== "completed" && status !== "cancelled" ? (
        <p className="export__note" role="note">
          {note}
          <button type="button" className="export__dismiss" onClick={dismissNote} aria-label="Dismiss note">
            <Icon name="x" size={11} />
          </button>
        </p>
      ) : null}

      <div className="export__actions">
        <button
          type="button"
          className="button button--primary button--sm"
          onClick={start}
          disabled={!canExport || saving}
        >
          <Icon name="download" size={12} />
          {isZip ? "Export ZIP" : "Export file"}
        </button>
        {saving ? (
          <button
            type="button"
            className="button button--ghost button--sm"
            onClick={cancel}
            disabled={cancelling}
          >
            {cancelling ? "Cancelling…" : "Cancel export"}
          </button>
        ) : null}
        <span className="export__count">
          {scope === "selection"
            ? `${selection.length} selected`
            : isZip
              ? plan
                ? `${plan.files.length} in review`
                : "Not prepared yet"
              : previewPath
                ? "One file"
                : "No file previewed"}
        </span>
      </div>
    </section>
  );
}
