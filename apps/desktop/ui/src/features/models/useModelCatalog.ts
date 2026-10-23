import { useCallback, useEffect, useRef, useState } from "react";
import type { ModelCatalog as CatalogPayload, ModelCatalogEntry } from "../../lib/ipc";
import { fetchModelCatalog, selectModel, toInferenceError } from "../../lib/inference";
import { isApprovedProfileId } from "./readiness";

export interface ModelCatalog {
  entries: ModelCatalogEntry[];
  /** The selected row, or null while loading or when no approved profile matches. */
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
 * because a profile exists, never accepts unapproved model IDs, never falls
 * back to another model when a selection is missing/mismatched, and discards
 * stale out-of-order asynchronous responses via monotonic sequence counters.
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

  const mountedRef = useRef(true);
  const fetchSeqRef = useRef(0);
  const selectSeqRef = useRef(0);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    const seq = ++fetchSeqRef.current;

    setLoading(true);

    fetchModelCatalog()
      .then((next: CatalogPayload) => {
        if (!mountedRef.current || seq !== fetchSeqRef.current) return;

        const rawEntries = Array.isArray(next?.entries) ? next.entries : [];
        const approvedEntries = rawEntries.filter((entry) =>
          isApprovedProfileId(entry?.profile?.id),
        );
        const hasApprovedSelection = approvedEntries.some((entry) => entry.selected);
        const derivedSelectionError =
          next?.selectionError ??
          (rawEntries.length > 0 && !hasApprovedSelection
            ? "The configured model profile is unrecognized or unavailable. Select Nexora Mini or Nexora Sonnet before sending."
            : null);

        setEntries(approvedEntries);
        setSelectionError(derivedSelectionError);
        setRunning(Boolean(next?.running));
        setLoadedProfileId(
          isApprovedProfileId(next?.loadedProfileId) ? (next?.loadedProfileId ?? null) : null,
        );
        setError(null);
      })
      .catch((cause: unknown) => {
        if (!mountedRef.current || seq !== fetchSeqRef.current) return;

        // A browser preview has no core; clear stale state instead of guessing.
        setEntries([]);
        setSelectionError(null);
        setRunning(false);
        setLoadedProfileId(null);
        setError(toInferenceError(cause).message);
      })
      .finally(() => {
        if (mountedRef.current && seq === fetchSeqRef.current) {
          setLoading(false);
        }
      });
  }, [attempt, refreshKey]);

  const reload = useCallback(() => {
    setAttempt((value) => value + 1);
  }, []);

  const dismissSwitchError = useCallback(() => setSwitchError(null), []);

  const select = useCallback(async (id: string) => {
    const trimmed = id.trim();
    const mySeq = ++selectSeqRef.current;

    if (!isApprovedProfileId(trimmed)) {
      if (mountedRef.current && mySeq === selectSeqRef.current) {
        setSwitchError(`No approved model profile is registered as "${trimmed || id}".`);
        setSelecting(false);
      }
      return;
    }

    setSelecting(true);

    try {
      await selectModel(trimmed);
      if (!mountedRef.current || mySeq !== selectSeqRef.current) return;
      setSwitchError(null);
      // Re-read rather than assuming: the core recomputes every status.
      setAttempt((value) => value + 1);
    } catch (cause: unknown) {
      if (!mountedRef.current || mySeq !== selectSeqRef.current) return;
      // The core owns this rule. Showing its sentence keeps the selector from
      // looking switched when the selection never changed.
      setSwitchError(toInferenceError(cause).message);
      setAttempt((value) => value + 1);
    } finally {
      if (mountedRef.current && mySeq === selectSeqRef.current) {
        setSelecting(false);
      }
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
