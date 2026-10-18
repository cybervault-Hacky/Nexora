import { useCallback, useEffect, useRef, useState } from "react";
import { getChangeImpact, type ImpactReport } from "../../lib/analysis";

export type ImpactState =
  | { status: "idle" }
  | { status: "loading"; path: string }
  | { status: "loaded"; report: ImpactReport }
  | { status: "error"; message: string; path?: string }
  | { status: "cancelled"; path?: string };

const IMPACT_TIMEOUT_MS = 3500;

export function useImpact(workspacePath: string | null) {
  const [impactState, setImpactState] = useState<ImpactState>({ status: "idle" });
  const requestIdRef = useRef(0);
  const abortRef = useRef<AbortController | null>(null);
  const timeoutRef = useRef<number | null>(null);
  const pathRef = useRef<string | null>(null);

  useEffect(() => {
    setImpactState({ status: "idle" });
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
    setImpactState({ status: "cancelled", path: p ?? undefined });
  }, []);

  const analyze = useCallback(
    async (path: string) => {
      if (!workspacePath) {
        setImpactState({ status: "error", message: "No project selected.", path });
        return;
      }
      if (!path.trim()) {
        setImpactState({ status: "error", message: "No file selected.", path });
        return;
      }
      const id = ++requestIdRef.current;
      pathRef.current = path;
      abortRef.current?.abort();
      if (timeoutRef.current) window.clearTimeout(timeoutRef.current);
      const controller = new AbortController();
      abortRef.current = controller;
      setImpactState({ status: "loading", path });
      const timeout = window.setTimeout(() => {
        controller.abort();
        if (requestIdRef.current === id) {
          setImpactState({ status: "error", message: "Impact analysis timed out.", path });
        }
      }, IMPACT_TIMEOUT_MS);
      timeoutRef.current = timeout as unknown as number;

      try {
        const report = await getChangeImpact(workspacePath, path);
        window.clearTimeout(timeout);
        timeoutRef.current = null;
        if (controller.signal.aborted || requestIdRef.current !== id) return;
        if (pathRef.current !== path) return;
        if (report.target !== path) {
          setImpactState({ status: "error", message: "Impact report does not match requested file.", path });
          return;
        }
        setImpactState({ status: "loaded", report });
      } catch (cause) {
        window.clearTimeout(timeout);
        timeoutRef.current = null;
        if (controller.signal.aborted || requestIdRef.current !== id) {
          if (requestIdRef.current === id) setImpactState({ status: "cancelled", path });
          return;
        }
        const msg = (cause as { message?: string })?.message ?? String(cause);
        setImpactState({ status: "error", message: msg, path });
      } finally {
        if (requestIdRef.current === id) abortRef.current = null;
      }
    },
    [workspacePath],
  );

  const reset = useCallback(() => {
    cancel();
    setImpactState({ status: "idle" });
  }, [cancel]);

  return { impactState, analyze, cancel, reset };
}
