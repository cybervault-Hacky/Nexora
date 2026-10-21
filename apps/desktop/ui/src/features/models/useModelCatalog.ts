import { useCallback, useEffect, useState } from "react";
import type { ModelCatalog as CatalogPayload, ModelCatalogEntry } from "../../lib/ipc";
import { fetchModelCatalog, selectModel, toInferenceError } from "../../lib/inference";

export interface ModelCatalog {
  entries: ModelCatalogEntry[];
  /** The selected row, or null while loading or when the core is unavailable. */
  selected: ModelCatalogEntry | null;
  loading: boolean;
  error: string | null;
  /** Set by the core when sending would be refused because of the selection. */
  selectionError: string | null;
  running: boolean;
  /** The profile the engine loaded, when the file name matched one. */
  loadedProfileId: string | null;
  /** True while a selection is being persisted. */
  selecting: boolean;
  /**
   * Why the last switch was refused, verbatim from the core. A switch is
   * refused while the engine is loading or answering; the message says which.
   */
  switchError: string | null;
  dismissSwitchError: () => void;
  select: (id: string) => Promise<void>;
  reload: () => void;
}

/**
 * Reads the model selector's data from the desktop core.
 *
 * The core owns every availability decision. This hook only caches the answer
 * and forwards an explicit selection; it never infers that a model is ready
 * because a profile exists, and it never falls back to another model.
 */
export function useModelCatalog(refreshKey = 0): ModelCatalog {
  const [entries, setEntries] = useState<ModelCatalogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectionError, setSelectionError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [loadedProfileId, setLoadedProfileId] = useState<string | null>(null);
  const [selecting, setSelecting] = useState(false);
  const [switchError, setSwitchError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;

    setLoading(true);

    fetchModelCatalog()
      .then((next: CatalogPayload) => {
        if (!active) return;

        setEntries(Array.isArray(next?.entries) ? next.entries : []);
        setSelectionError(next?.selectionError ?? null);
        setRunning(Boolean(next?.running));
        setLoadedProfileId(next?.loadedProfileId ?? null);
        setError(null);
      })
      .catch((cause: unknown) => {
        if (!active) return;

        // A browser preview has no core; that is not an error to shout about.
        setEntries([]);
        setSelectionError(null);
        setRunning(false);
        setLoadedProfileId(null);
        setError(toInferenceError(cause).message);
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, [attempt, refreshKey]);

  const reload = useCallback(() => {
    setAttempt((value) => value + 1);
  }, []);

  const dismissSwitchError = useCallback(() => setSwitchError(null), []);

  const select = useCallback(async (id: string) => {
    setSelecting(true);

    try {
      await selectModel(id);
      setSwitchError(null);
      // Re-read rather than assuming: the core recomputes every status.
      setAttempt((value) => value + 1);
    } catch (cause: unknown) {
      // The core owns this rule. Showing its sentence keeps the selector from
      // looking switched when the selection never changed.
      setSwitchError(toInferenceError(cause).message);
      setAttempt((value) => value + 1);
    } finally {
      setSelecting(false);
    }
  }, []);

  return {
    entries,
    selected: entries.find((entry) => entry.selected) ?? null,
    loading,
    error,
    selectionError,
    running,
    loadedProfileId,
    selecting,
    switchError,
    dismissSwitchError,
    select,
    reload,
  };
}
