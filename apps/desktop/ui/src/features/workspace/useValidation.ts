import { useCallback, useEffect, useRef, useState } from "react";
import { validatePatchDetailed, type ValidationReport } from "../../lib/validation";
import type { PatchProposal } from "../../lib/patch";

export type ValidationState =
  | { status: "idle" }
  | { status: "validating"; proposalId: string }
  | { status: "validated"; report: ValidationReport }
  | { status: "error"; message: string; proposalId?: string }
  | { status: "cancelled"; proposalId?: string };

const VALIDATION_TIMEOUT_MS = 3000;

export function useValidation(workspacePath: string | null) {
  const [state, setState] = useState<ValidationState>({ status: "idle" });
  const currentProposalIdRef = useRef<string | null>(null);
  const timeoutRef = useRef<number | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  // Clear on workspace switch or proposal change is handled by caller via reset
  useEffect(() => {
    // workspace switch clears validation
    setState({ status: "idle" });
    currentProposalIdRef.current = null;
    if (timeoutRef.current) {
      window.clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    abortRef.current?.abort();
    abortRef.current = null;
  }, [workspacePath]);

  const cancel = useCallback(() => {
    abortRef.current?.abort();
    if (timeoutRef.current) {
      window.clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    const pid = currentProposalIdRef.current;
    currentProposalIdRef.current = null;
    setState(pid ? { status: "cancelled", proposalId: pid } : { status: "idle" });
  }, []);

  const reset = useCallback(() => {
    cancel();
    setState({ status: "idle" });
  }, [cancel]);

  const validate = useCallback(
    async (proposal: PatchProposal) => {
      if (!workspacePath) {
        setState({ status: "error", message: "No project selected." });
        return;
      }
      // Prevent stale overwrite: if already validating same proposal, ignore
      if (currentProposalIdRef.current === proposal.id && state.status === "validating") {
        return;
      }
      // Cancel previous
      abortRef.current?.abort();
      if (timeoutRef.current) window.clearTimeout(timeoutRef.current);

      currentProposalIdRef.current = proposal.id;
      setState({ status: "validating", proposalId: proposal.id });
      const controller = new AbortController();
      abortRef.current = controller;

      // Timeout
      const timeout = window.setTimeout(() => {
        controller.abort();
        // Mark as error with timeout message, but only if still validating same proposal
        if (currentProposalIdRef.current === proposal.id) {
          setState({ status: "error", message: "Validation timed out — checks took too long.", proposalId: proposal.id });
          currentProposalIdRef.current = null;
        }
      }, VALIDATION_TIMEOUT_MS);
      timeoutRef.current = timeout as unknown as number;

      try {
        // Race with abort signal — our validatePatchDetailed doesn't accept signal, so we handle via abort check after await
        const report = await validatePatchDetailed(workspacePath, proposal);
        window.clearTimeout(timeout);
        timeoutRef.current = null;
        if (controller.signal.aborted) {
          // Stale result, ignore
          return;
        }
        // Ensure still the current proposal (workspace switch or new proposal)
        if (currentProposalIdRef.current !== proposal.id) {
          return; // stale
        }
        // Verify report matches proposal (association)
        if (report.proposalId !== proposal.id) {
          setState({ status: "error", message: "Validation report does not match proposal.", proposalId: proposal.id });
          return;
        }
        setState({ status: "validated", report });
      } catch (cause) {
        window.clearTimeout(timeout);
        timeoutRef.current = null;
        if (controller.signal.aborted) {
          // Already handled as cancelled/timeout
          if (currentProposalIdRef.current === proposal.id) {
            // If we aborted via timeout, keep error; else cancelled
            // Check if we already set timeout error
            setState((prev) => (prev.status === "error" && prev.message.includes("timed out") ? prev : { status: "cancelled", proposalId: proposal.id }));
          }
          return;
        }
        const msg = (cause as { message?: string })?.message ?? String(cause);
        if (currentProposalIdRef.current !== proposal.id) return; // stale
        setState({ status: "error", message: msg, proposalId: proposal.id });
      } finally {
        if (currentProposalIdRef.current === proposal.id) {
          // Keep ref until next validation, but clear abort
          abortRef.current = null;
        }
      }
    },
    [workspacePath, state.status],
  );

  return {
    validationState: state,
    validate,
    cancel,
    reset,
  };
}
