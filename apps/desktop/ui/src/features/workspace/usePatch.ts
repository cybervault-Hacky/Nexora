import { useCallback, useEffect, useState } from "react";
import {
  createPatchProposal,
  validatePatchProposal,
  applyPatchProposal,
  getPatchDiff,
  type PatchProposal,
  type PatchValidation,
  type ApplyResult,
  type PatchDiffSummary,
} from "../../lib/patch";

export type PatchState =
  | { status: "idle" }
  | { status: "editing"; path: string; originalContent: string; proposedContent: string }
  | { status: "creating" }
  | { status: "preview"; proposal: PatchProposal; validation: PatchValidation; diff: PatchDiffSummary }
  | { status: "validating"; proposal: PatchProposal }
  | { status: "applying"; proposal: PatchProposal }
  | { status: "applied"; result: ApplyResult; proposal: PatchProposal }
  | { status: "error"; message: string; proposal?: PatchProposal };

export function usePatch(workspacePath: string | null) {
  const [state, setState] = useState<PatchState>({ status: "idle" });
  const [editorContent, setEditorContent] = useState("");

  // Invalidate proposal when workspace switches
  useEffect(() => {
    setState({ status: "idle" });
    setEditorContent("");
  }, [workspacePath]);

  const startEditing = useCallback((path: string, originalContent: string) => {
    setEditorContent(originalContent);
    setState({ status: "editing", path, originalContent, proposedContent: originalContent });
  }, []);

  const updateEditor = useCallback((value: string) => {
    setEditorContent(value);
    setState((prev) => (prev.status === "editing" ? { ...prev, proposedContent: value } : prev));
  }, []);

  const cancel = useCallback(() => {
    setState({ status: "idle" });
    setEditorContent("");
  }, []);

  const createProposal = useCallback(async () => {
    if (!workspacePath) {
      setState({ status: "error", message: "No project selected." });
      return;
    }
    if (state.status !== "editing") {
      setState({ status: "error", message: "No file is being edited." });
      return;
    }
    if (state.proposedContent === state.originalContent) {
      setState({ status: "error", message: "No changes detected — edit the file to create a proposal.", proposal: undefined });
      return;
    }
    setState({ status: "creating" });
    try {
      const proposal = await createPatchProposal(workspacePath, state.path, state.proposedContent);
      // Validate immediately for UI
      const validation = await validatePatchProposal(workspacePath, proposal);
      const diff = await getPatchDiff(proposal.originalContent, proposal.proposedContent);
      setState({ status: "preview", proposal, validation: { ...validation, diffSummary: diff }, diff });
    } catch (cause) {
      const msg = (cause as { message?: string })?.message ?? String(cause);
      setState({ status: "error", message: msg });
    }
  }, [workspacePath, state]);

  const revalidate = useCallback(async () => {
    if (!workspacePath || (state.status !== "preview" && state.status !== "applied")) return;
    const proposal = (state as { proposal: PatchProposal }).proposal;
    if (!proposal) return;
    setState({ status: "validating", proposal });
    try {
      const validation = await validatePatchProposal(workspacePath, proposal);
      const diff = await getPatchDiff(proposal.originalContent, proposal.proposedContent);
      setState({ status: "preview", proposal, validation: { ...validation, diffSummary: diff }, diff });
    } catch (cause) {
      const msg = (cause as { message?: string })?.message ?? String(cause);
      setState({ status: "error", message: msg, proposal });
    }
  }, [workspacePath, state]);

  const apply = useCallback(async () => {
    if (!workspacePath) {
      setState({ status: "error", message: "No project selected." });
      return;
    }
    if (state.status !== "preview") {
      setState({ status: "error", message: "No proposal to apply." });
      return;
    }
    if (!state.validation.ok) {
      setState({ status: "error", message: `Proposal invalid: ${state.validation.errors.join("; ")}`, proposal: state.proposal });
      return;
    }
    if (state.validation.warnings.some((w) => w.toLowerCase().includes("stale"))) {
      setState({ status: "error", message: "File has changed since proposal was created. Reload the file and recreate the proposal.", proposal: state.proposal });
      return;
    }
    setState({ status: "applying", proposal: state.proposal });
    try {
      const result = await applyPatchProposal(workspacePath, state.proposal);
      setState({ status: "applied", result, proposal: state.proposal });
    } catch (cause) {
      const msg = (cause as { message?: string })?.message ?? String(cause);
      // Check if stale
      if (msg.toLowerCase().includes("stale")) {
        setState({ status: "error", message: msg, proposal: state.proposal });
      } else {
        setState({ status: "error", message: msg, proposal: state.proposal });
      }
    }
  }, [workspacePath, state]);

  return {
    patchState: state,
    editorContent,
    startEditing,
    updateEditor,
    cancel,
    createProposal,
    revalidate,
    apply,
    setState, // for tests
  };
}
