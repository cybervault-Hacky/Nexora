import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  cancelExport as cancelExportCommand,
  exportProjectZip,
  exportSingleFile,
  planWorkspaceExport,
  suggestExportPath,
  toExportError,
  type ExportError,
  type ExportEvent,
  type ExportOutcome,
  type ExportPlan,
  type ExportProgress,
  type SingleExportRequest,
  type ZipExportRequest,
} from "../../lib/export";

export type ExportScope = "project" | "selection" | "file";

export type ExportStatus =
  | "idle"
  | "preparing"
  | "ready"
  | "saving"
  | "completed"
  | "cancelled"
  | "failed";

interface Options {
  workspacePath: string | null;
  selectedPaths: ReadonlySet<string>;
  previewPath: string | null;
}

interface Pending {
  kind: "file" | "zip";
  request: SingleExportRequest | ZipExportRequest;
}

export interface ExportController {
  scope: ExportScope;
  setScope: (scope: ExportScope) => void;
  status: ExportStatus;
  plan: ExportPlan | null;
  destination: string;
  setDestination: (value: string) => void;
  progress: ExportProgress | null;
  outcome: ExportOutcome | null;
  error: ExportError | null;
  conflict: string | null;
  note: string | null;
  planNote: string | null;
  cancelling: boolean;
  selection: string[];
  canExport: boolean;
  chooseDestination: () => void;
  start: () => void;
  cancel: () => void;
  replace: () => void;
  dismissNote: () => void;
}

const SAVE_NOTE =
  "The native save dialog is not installed in this build, so type the destination path. Exports are still validated and written by the desktop core.";

export function useExport({ workspacePath, selectedPaths, previewPath }: Options): ExportController {
  const [scope, setScopeState] = useState<ExportScope>("project");
  const [status, setStatus] = useState<ExportStatus>("idle");
  const [plan, setPlan] = useState<ExportPlan | null>(null);
  const [destination, setDestinationState] = useState("");
  const [progress, setProgress] = useState<ExportProgress | null>(null);
  const [outcome, setOutcome] = useState<ExportOutcome | null>(null);
  const [error, setError] = useState<ExportError | null>(null);
  const [conflict, setConflict] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [planNote, setPlanNote] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);

  const selection = useMemo(() => [...selectedPaths].sort(), [selectedPaths]);
  const selectionKey = selection.join("\u0000");
  const selectionRef = useRef<string[]>(selection);
  selectionRef.current = selection;

  const mounted = useRef(true);
  const busy = useRef(false);
  const pending = useRef<Pending | null>(null);
  const touchedDestination = useRef(false);
  const previousRoot = useRef<string | null>(null);

  useEffect(() => {
    mounted.current = true;

    return () => {
      mounted.current = false;
    };
  }, []);

  const setDestination = useCallback((value: string) => {
    touchedDestination.current = true;
    pending.current = null;
    setConflict(null);
    setDestinationState(value);
  }, []);

  const setScope = useCallback((value: ExportScope) => {
    touchedDestination.current = false;
    setDestinationState("");
    setScopeState(value);
    setOutcome(null);
    setError(null);
    setConflict(null);
    setNote(null);
    setPlanNote(null);
    setProgress(null);
  }, []);

  // A new project resets every piece of per-export state.
  useEffect(() => {
    if (previousRoot.current === workspacePath) {
      return;
    }

    previousRoot.current = workspacePath;
    touchedDestination.current = false;
    setScopeState("project");
    setDestinationState("");
    setPlan(null);
    setOutcome(null);
    setError(null);
    setConflict(null);
    setNote(null);
    setPlanNote(null);
    setProgress(null);
    setStatus(workspacePath ? "preparing" : "idle");
  }, [workspacePath]);

  // Review list for the chosen scope. The single-file scope needs no scan.
  useEffect(() => {
    if (!workspacePath || scope === "file" || busy.current) {
      return;
    }

    let active = true;
    setStatus("preparing");
    setPlan(null);
    setPlanNote(null);
    setError(null);

    planWorkspaceExport(scope === "selection" ? selectionRef.current : null)
      .then((value) => {
        if (!active) return;

        if (!value || !Array.isArray(value.files)) {
          setError({
            code: "protocol",
            message: "The desktop core returned an unexpected export plan.",
          });
          setStatus("failed");
          return;
        }

        setPlan(value);
        setStatus("ready");
      })
      .catch((cause) => {
        if (!active) return;

        const failure = toExportError(cause);

        if (failure.code === "no_files") {
          setPlan(null);
          setPlanNote(failure.message);
          setStatus("ready");
          return;
        }

        setError(failure);
        setStatus("failed");
      });

    return () => {
      active = false;
    };
  }, [workspacePath, scope, selectionKey]);

  // The single-file scope needs no scan: it is ready as soon as a file is
  // previewed, and idle otherwise.
  useEffect(() => {
    if (!workspacePath || scope !== "file") {
      return;
    }

    setPlan(null);
    setStatus(previewPath ? "ready" : "idle");
  }, [workspacePath, scope, previewPath]);

  // Default destination, unless the user has typed or picked one.
  useEffect(() => {
    if (!workspacePath || touchedDestination.current) {
      return;
    }

    let active = true;
    const kind = scope === "file" ? "file" : "zip";

    suggestExportPath(kind, scope === "file" ? previewPath : null)
      .then((value) => {
        if (active && !touchedDestination.current && typeof value === "string") {
          setDestinationState(value);
        }
      })
      .catch(() => {
        // The user can still type a path; suggest is a convenience.
      });

    return () => {
      active = false;
    };
  }, [workspacePath, scope, previewPath]);

  const canExport = Boolean(
    workspacePath &&
      destination.trim() &&
      status !== "saving" &&
      status !== "preparing" &&
      !conflict &&
      (scope === "file" ? previewPath : plan && plan.files.length > 0),
  );

  const handleEvent = useCallback((event: ExportEvent) => {
    if (event.type === "progress") {
      setProgress(event);
    }
  }, []);

  const run = useCallback(
    async (kind: "file" | "zip", request: SingleExportRequest | ZipExportRequest) => {
      busy.current = true;
      pending.current = null;
      setStatus("saving");
      setCancelling(false);
      setError(null);
      setOutcome(null);
      setProgress(null);

      try {
        const result =
          kind === "file"
            ? await exportSingleFile(request as SingleExportRequest, handleEvent)
            : await exportProjectZip(request as ZipExportRequest, handleEvent);

        if (!mounted.current) return;

        setOutcome(result);
        setStatus(result.cancelled ? "cancelled" : "completed");
      } catch (cause) {
        if (!mounted.current) return;

        const failure = toExportError(cause);

        if (failure.code === "already_exists") {
          pending.current = { kind, request };
          setConflict(failure.message);
          setStatus("ready");
        } else if (failure.code === "cancelled") {
          setStatus("cancelled");
        } else {
          setError(failure);
          setStatus("failed");
        }
      } finally {
        busy.current = false;
        if (mounted.current) setCancelling(false);
      }
    },
    [handleEvent],
  );

  const start = useCallback(() => {
    if (!workspacePath || busy.current) return;

    if (scope === "file" && !previewPath) {
      setError({
        code: "invalid_request",
        message: "Preview a file before exporting a single file.",
      });
      setStatus("failed");
      return;
    }

    const request: SingleExportRequest | ZipExportRequest =
      scope === "file"
        ? {
            relativePath: previewPath as string,
            destination: destination.trim(),
            overwrite: false,
          }
        : {
            selection: scope === "selection" ? selection : null,
            destination: destination.trim(),
            overwrite: false,
          };

    void run(scope === "file" ? "file" : "zip", request);
  }, [workspacePath, scope, previewPath, destination, selection, run]);

  const cancel = useCallback(() => {
    setCancelling(true);
    void cancelExportCommand();
  }, []);

  const replace = useCallback(() => {
    const request = pending.current;

    if (!request || busy.current) return;

    setConflict(null);

    void run(request.kind, { ...request.request, overwrite: true });
  }, [run]);

  const chooseDestination = useCallback(() => {
    const pick = async () => {
      const spec = "@tauri-apps" + "/plugin-dialog";

      try {
        const module = (await import(/* @vite-ignore */ spec)) as {
          save?: (options: unknown) => Promise<string | null>;
        };

        if (typeof module.save === "function") {
          const options =
            scope === "file"
              ? { defaultPath: destination || undefined }
              : {
                  defaultPath: destination || undefined,
                  filters: [{ name: "ZIP archive", extensions: ["zip"] }],
                };

          const picked = await module.save(options);

          if (typeof picked === "string" && picked.trim()) {
            setDestination(picked);
          }

          return;
        }
      } catch {
        // The optional dialog plugin is not installed in this build.
      }

      setNote(SAVE_NOTE);
    };

    void pick();
  }, [destination, scope, setDestination]);

  const dismissNote = useCallback(() => setNote(null), []);

  return {
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
  };
}
