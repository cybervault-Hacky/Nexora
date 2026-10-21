import type { ModelCatalogEntry } from "../lib/ipc";
import type { ModelCatalog } from "../features/models/useModelCatalog";
import { readinessLabel } from "../features/models/readiness";

interface ModelSelectorProps {
  catalog: ModelCatalog;
  /** True while a reply streams: switching mid-answer is not allowed. */
  disabled?: boolean;
}

/**
 * The option text uses the backend's state name, so a model can never read
 * "Ready" here unless the core said `loaded` or `ready_to_load`.
 */
function describeOption(entry: ModelCatalogEntry): string {
  return `${entry.profile.displayName} — ${readinessLabel(entry.status)}`;
}

/**
 * Compact model switch for the chat composer.
 *
 * Deliberately not a page or a dashboard: one labelled `<select>` plus one line
 * of status. Options and their wording come from the core's catalogue, so a
 * model whose artefact is not approved reads "Unavailable" and can never be
 * shown as ready.
 */
export function ModelSelector({ catalog, disabled = false }: ModelSelectorProps) {
  const { entries, selected, selecting, error, selectionError, switchError } =
    catalog;

  if (entries.length === 0) {
    return null;
  }

  const current = selected ?? entries[0];
  const busy = disabled || selecting;
  // The backend's own sentence. `loaded` still has something useful to say,
  // and every other state explains what is missing or what to do next.
  const status = current?.readiness?.detail ?? current?.detail ?? null;
  const tone = current ? current.readiness?.state : undefined;

  // A refused switch outranks the readiness sentence: it says what the user
  // just tried to do and why it did not happen. The selection never changed.
  const notice = switchError ?? selectionError ?? null;

  return (
    <div className="model-select">
      <div className="model-select__row">
        <label className="model-select__label" htmlFor="model-select">
          Model
        </label>

        <select
          id="model-select"
          className="model-select__control"
          value={current?.profile.id ?? ""}
          disabled={busy}
          aria-describedby="model-select-status"
          title={
            disabled ? "Stop generating before switching models" : "Choose the local model"
          }
          onChange={(event) => {
            void catalog.select(event.target.value);
          }}
        >
          {entries.map((entry) => (
            <option key={entry.profile.id} value={entry.profile.id}>
              {describeOption(entry)}
            </option>
          ))}
        </select>
      </div>

      <p
        className={`model-select__status${
          notice ? " model-select__status--warning" : ""
        }`}
        id="model-select-status"
        aria-live="polite"
        data-state={tone}
        role={notice ? "status" : undefined}
      >
        {error ? `Model list unavailable: ${error}` : (notice ?? status ?? "")}
      </p>
    </div>
  );
}
