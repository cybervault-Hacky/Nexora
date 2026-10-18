import { useCallback, useEffect, useRef, useState } from "react";
import {
  getDependencyGraph,
  getFileRelationships,
  type AnalysisGraph,
  type FileRelationships,
} from "../../lib/analysis";

export type AnalysisState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "loaded"; graph: AnalysisGraph }
  | { status: "error"; message: string }
  | { status: "cancelled" };

export type RelationshipsState =
  | { status: "idle" }
  | { status: "loading"; path: string }
  | { status: "loaded"; data: FileRelationships }
  | { status: "error"; message: string; path?: string }
  | { status: "cancelled"; path?: string };

const GRAPH_TIMEOUT_MS = 4000;
const REL_TIMEOUT_MS = 3000;

export function useAnalysis(workspacePath: string | null) {
  const [analysisState, setAnalysisState] = useState<AnalysisState>({ status: "idle" });
  const requestIdRef = useRef(0);
  const abortRef = useRef<AbortController | null>(null);
  const timeoutRef = useRef<number | null>(null);

  useEffect(() => {
    // workspace switch clears
    setAnalysisState({ status: "idle" });
    requestIdRef.current += 1;
    abortRef.current?.abort();
    abortRef.current = null;
    if (timeoutRef.current) {
      window.clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
  }, [workspacePath]);

  const cancel = useCallback(() => {
    abortRef.current?.abort();
    if (timeoutRef.current) {
      window.clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    requestIdRef.current += 1;
    setAnalysisState({ status: "cancelled" });
  }, []);

  const refresh = useCallback(async () => {
    if (!workspacePath) {
      setAnalysisState({ status: "error", message: "No project selected." });
      return;
    }
    const id = ++requestIdRef.current;
    abortRef.current?.abort();
    if (timeoutRef.current) window.clearTimeout(timeoutRef.current);
    const controller = new AbortController();
    abortRef.current = controller;
    setAnalysisState({ status: "loading" });
    const timeout = window.setTimeout(() => {
      controller.abort();
      if (requestIdRef.current === id) {
        setAnalysisState({ status: "error", message: "Analysis timed out — graph too large or still building." });
      }
    }, GRAPH_TIMEOUT_MS);
    timeoutRef.current = timeout as unknown as number;

    try {
      const graph = await getDependencyGraph(workspacePath);
      window.clearTimeout(timeout);
      timeoutRef.current = null;
      if (controller.signal.aborted || requestIdRef.current !== id) return;
      setAnalysisState({ status: "loaded", graph });
    } catch (cause) {
      window.clearTimeout(timeout);
      timeoutRef.current = null;
      if (controller.signal.aborted || requestIdRef.current !== id) {
        if (requestIdRef.current === id) setAnalysisState({ status: "cancelled" });
        return;
      }
      const msg = (cause as { message?: string })?.message ?? String(cause);
      setAnalysisState({ status: "error", message: msg });
    } finally {
      if (requestIdRef.current === id) abortRef.current = null;
    }
  }, [workspacePath]);

  const reset = useCallback(() => {
    cancel();
    setAnalysisState({ status: "idle" });
  }, [cancel]);

  return { analysisState, refresh, cancel, reset };
}

export function useRelationships(workspacePath: string | null) {
  const [relState, setRelState] = useState<RelationshipsState>({ status: "idle" });
  const requestIdRef = useRef(0);
  const abortRef = useRef<AbortController | null>(null);
  const timeoutRef = useRef<number | null>(null);
  const pathRef = useRef<string | null>(null);

  useEffect(() => {
    setRelState({ status: "idle" });
    requestIdRef.current += 1;
    pathRef.current = null;
    abortRef.current?.abort();
    abortRef.current = null;
    if (timeoutRef.current) {
      window.clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
  }, [workspacePath]);

  const cancel = useCallback(() => {
    abortRef.current?.abort();
    if (timeoutRef.current) {
      window.clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    const p = pathRef.current;
    requestIdRef.current += 1;
    setRelState({ status: "cancelled", path: p ?? undefined });
  }, []);

  const load = useCallback(
    async (path: string) => {
      if (!workspacePath) {
        setRelState({ status: "error", message: "No project selected.", path });
        return;
      }
      if (!path.trim()) {
        setRelState({ status: "error", message: "No file selected.", path });
        return;
      }
      const id = ++requestIdRef.current;
      pathRef.current = path;
      abortRef.current?.abort();
      if (timeoutRef.current) window.clearTimeout(timeoutRef.current);
      const controller = new AbortController();
      abortRef.current = controller;
      setRelState({ status: "loading", path });
      const timeout = window.setTimeout(() => {
        controller.abort();
        if (requestIdRef.current === id) {
          setRelState({ status: "error", message: "Relationship analysis timed out.", path });
        }
      }, REL_TIMEOUT_MS);
      timeoutRef.current = timeout as unknown as number;

      try {
        const data = await getFileRelationships(workspacePath, path);
        window.clearTimeout(timeout);
        timeoutRef.current = null;
        if (controller.signal.aborted || requestIdRef.current !== id) return;
        if (pathRef.current !== path) return; // stale
        // association check
        if (data.path !== path) {
          // still show but note mismatch? For now error
          setRelState({ status: "error", message: "Relationship data does not match requested file.", path });
          return;
        }
        setRelState({ status: "loaded", data });
      } catch (cause) {
        window.clearTimeout(timeout);
        timeoutRef.current = null;
        if (controller.signal.aborted || requestIdRef.current !== id) {
          if (requestIdRef.current === id) setRelState({ status: "cancelled", path });
          return;
        }
        const msg = (cause as { message?: string })?.message ?? String(cause);
        setRelState({ status: "error", message: msg, path });
      } finally {
        if (requestIdRef.current === id) abortRef.current = null;
      }
    },
    [workspacePath],
  );

  const reset = useCallback(() => {
    cancel();
    setRelState({ status: "idle" });
  }, [cancel]);

  return { relState, load, cancel, reset };
}
