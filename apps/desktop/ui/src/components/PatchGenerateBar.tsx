import { useEffect, useState } from "react";
import { Icon } from "./Icon";
import { PatchPreview } from "./PatchPreview";
import { ValidationPanel } from "./ValidationPanel";
import { usePatchGeneration } from "../features/workspace/usePatchGeneration";
import { useValidation } from "../features/workspace/useValidation";
import { usePatchRepair } from "../features/workspace/usePatchRepair";
import type { AppStatus } from "../lib/ipc";
import type { SymbolIndex } from "../lib/symbolIndex";

interface Props {
  workspacePath: string | null;
  targetPath: string | null;
  originalContent: string | null;
  filteredContextBlock: string | null;
  engineStatus: AppStatus | null;
  symbolIndex?: SymbolIndex | null;
  contextFiles?: Array<{ path: string; content: string; size: number }>;
  onApplied?: () => void;
  onInvalidate?: () => void;
}

export function PatchGenerateBar({
  workspacePath,
  targetPath,
  originalContent,
  filteredContextBlock,
  engineStatus,
  symbolIndex,
  contextFiles,
  onApplied,
  onInvalidate,
}: Props) {
  const [instruction, setInstruction] = useState("");
  const generation = usePatchGeneration({ workspacePath, engineStatus });
  const validation = useValidation(workspacePath);
  const repair = usePatchRepair({
    workspacePath,
    engineStatus,
    symbolIndex: symbolIndex ?? null,
    contextFiles: contextFiles ?? [],
    originalInstruction: instruction,
  });

  // Invalidate when target changes
  useEffect(() => {
    generation.invalidate();
    // also notify parent to clear manual patch if needed
    onInvalidate?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [targetPath]);

  // Notify parent on applied
  useEffect(() => {
    if (generation.generationState.status === "applied") {
      onApplied?.();
    }
  }, [generation.generationState.status, onApplied]);

  // Phase 15: detailed validation for generated proposal (bounded, non-mutating)
  useEffect(() => {
    const s = generation.generationState;
    if (s.status === "preview" && s.proposal) {
      void validation.validate(s.proposal);
    } else if (s.status === "idle" || s.status === "error" || s.status === "cancelled") {
      validation.reset();
      if (s.status === "idle") repair.reset();
    }
  }, [generation.generationState]);

  const engineState = engineStatus?.engine.state ?? "not_configured";
  const isGenerating = generation.generationState.status === "generating" || generation.generationState.status === "parsing";

  const handleGenerate = () => {
    if (!targetPath || !originalContent) return;
    void generation.generatePatch({
      targetPath,
      originalContent,
      instruction,
      filteredContextBlock,
    });
  };

  const handleApply = () => {
    void generation.apply();
  };

  const handleDismiss = () => {
    generation.dismiss();
  };

  // State banners
  if (!workspacePath) {
    return (
      <div className="patch-generate" role="status" aria-label="Patch generation">
        <p className="patch-generate__hint">Open a project to generate patches.</p>
      </div>
    );
  }

  if (!targetPath || !originalContent) {
    return (
      <div className="patch-generate" role="status" aria-label="Patch generation">
        <p className="patch-generate__hint">Select one eligible file in the explorer to generate a patch. Click a file to preview, check to include in context.</p>
      </div>
    );
  }

  // Check file excluded quickly (sensitive handled in hook, but show early)
  const isSensitive = targetPath.split("/").pop() === ".env" || targetPath.endsWith(".pem");
  if (isSensitive) {
    return (
      <div className="patch-generate" role="alert" aria-label="Patch generation">
        <p className="patch-generate__error">File excluded for security reasons — cannot patch sensitive files.</p>
      </div>
    );
  }

  // Model readiness
  const modelNotReady = engineState === "not_configured" || engineState === "stopped" || engineState === "error";
  const modelLoading = engineState === "loading";

  // Render generation UI + preview when needed
  const state = generation.generationState;

  // If preview or applied, show explanation + PatchPreview + detailed validation
  if (state.status === "preview") {
    const isRevised = repair.repairState.status === "revised";
    const revised = isRevised ? (repair.repairState as any).proposal : null;
    const revisedValidation = isRevised ? (repair.repairState as any).validation : null;
    return (
      <div className="patch-generate" aria-label="Patch generation">
        {state.explanation ? (
          <div className="patch-generate__explanation" role="note" aria-label="Proposed change explanation">
            <Icon name="edit" size={12} />
            <span>{state.explanation}</span>
          </div>
        ) : null}
        <PatchPreview
          proposal={state.proposal}
          validation={state.validation}
          onCancel={handleDismiss}
          onApply={handleApply}
        />
        <ValidationPanel
          proposal={state.proposal}
          report={validation.validationState.status === "validated" ? validation.validationState.report : null}
          validating={validation.validationState.status === "validating"}
          error={validation.validationState.status === "error" ? validation.validationState.message : null}
          onValidate={() => void validation.validate(state.proposal)}
          onCancelValidate={validation.cancel}
          repairState={repair.repairState as any}
          onExplain={() => {
            const rep = validation.validationState.status === "validated" ? validation.validationState.report : null;
            if (rep) void repair.explain(state.proposal, rep);
          }}
          onRepair={() => {
            const rep = validation.validationState.status === "validated" ? validation.validationState.report : null;
            if (rep) void repair.repair(state.proposal, rep);
          }}
          onCancelRepair={repair.cancel}
          maxAttempts={repair.maxAttempts}
          revisedProposal={revised}
        />
        {isRevised && revised ? (
          <div className="patch-generate__revised" aria-label="Revised patch">
            <h4 className="validation__revised-header">Revised proposal (not yet applied)</h4>
            <PatchPreview
              proposal={revised}
              validation={revisedValidation}
              onCancel={repair.reset}
              onApply={async () => {
                // Apply revised explicitly — reuse generated apply path with new proposal
                // We create a temporary generation state for revised and call apply via direct patch apply
                const { applyPatchProposal } = await import("../lib/patch");
                if (!workspacePath) return;
                try {
                  await applyPatchProposal(workspacePath, revised);
                  repair.reset();
                  generation.dismiss();
                  onApplied?.();
                } catch {
                  // keep revised preview
                }
              }}
            />
            <p className="patch-generate__note">Revised proposal — not auto-applied. Review diff and validation above, then Apply revised.</p>
          </div>
        ) : null}
        <p className="patch-generate__note">Generated via local model — untrusted until validated. Original file unchanged until you apply.</p>
      </div>
    );
  }

  if (state.status === "applied") {
    return (
      <div className="patch-generate" aria-label="Patch generation">
        <PatchPreview proposal={state.proposal} validation={null} applied onCancel={handleDismiss} onApply={() => {}} />
        {state.explanation ? (
          <p className="patch-generate__explanation">{state.explanation}</p>
        ) : null}
      </div>
    );
  }

  if (state.status === "error") {
    return (
      <div className="patch-generate" aria-label="Patch generation">
        <div className="patch-generate__error" role="alert">
          <Icon name="alert" size={12} />
          <span>{state.message}</span>
        </div>
        <p className="patch-generate__hint">Try a more specific instruction, or check that the target file is eligible.</p>
        <div className="patch-generate__actions">
          <button type="button" className="button button--ghost button--sm" onClick={handleDismiss}>
            Dismiss
          </button>
          <button type="button" className="button button--primary button--sm" onClick={handleGenerate} disabled={isGenerating || !instruction.trim()}>
            Retry
          </button>
        </div>
      </div>
    );
  }

  if (state.status === "cancelled") {
    return (
      <div className="patch-generate" aria-label="Patch generation">
        <p className="patch-generate__hint">Generation cancelled.</p>
        <div className="patch-generate__actions">
          <button type="button" className="button button--ghost button--sm" onClick={handleDismiss}>
            Dismiss
          </button>
        </div>
      </div>
    );
  }

  if (state.status === "parsing") {
    return (
      <div className="patch-generate" role="status" aria-label="Patch generation">
        <p className="patch-generate__status">
          <span className="pulse" aria-hidden="true" />
          Validating model response…
        </p>
      </div>
    );
  }

  // Default idle / generating
  return (
    <div className="patch-generate" aria-label="Patch generation">
      <header className="patch-generate__header">
        <div className="patch-generate__title">
          <Icon name="edit" size={13} />
          <span>AI Patch — {targetPath}</span>
        </div>
        {modelNotReady ? (
          <span className="patch-generate__badge patch-generate__badge--warn">Model not ready</span>
        ) : modelLoading ? (
          <span className="patch-generate__badge">Loading…</span>
        ) : null}
      </header>

      {modelNotReady ? (
        <p className="patch-generate__hint">
          No model available — load a GGUF in Settings to enable patch generation. This will run locally via llama.cpp.
        </p>
      ) : modelLoading ? (
        <p className="patch-generate__hint">Model is loading — please wait.</p>
      ) : null}

      <label className="patch-generate__label" htmlFor="patch-instruction">
        Change request
      </label>
      <textarea
        id="patch-instruction"
        className="patch-generate__input"
        value={instruction}
        onChange={(e) => setInstruction(e.target.value)}
        placeholder="Describe the change you want, e.g., 'Refactor this function to use async/await and add error handling'"
        rows={3}
        maxLength={2000}
        disabled={isGenerating || modelNotReady || modelLoading}
        aria-label={`Change request for ${targetPath}`}
      />
      <div className="patch-generate__meta">
        <span>{instruction.length}/2000</span>
        {isGenerating ? <span className="patch-generate__status">Generating…</span> : null}
      </div>

      <div className="patch-generate__actions">
        {isGenerating ? (
          <button type="button" className="button button--ghost button--sm" onClick={() => void generation.cancel()}>
            Cancel
          </button>
        ) : (
          <button
            type="button"
            className="button button--primary button--sm"
            onClick={handleGenerate}
            disabled={!instruction.trim() || modelNotReady || modelLoading}
            title={
              modelNotReady
                ? "Load a model first"
                : !instruction.trim()
                  ? "Enter a change request"
                  : `Generate patch for ${targetPath} via local model`
            }
          >
            Generate patch
          </button>
        )}
      </div>

      {isGenerating ? (
        <div className="patch-generate__generating" role="status" aria-live="polite">
          <span className="pulse" aria-hidden="true" />
          <span>Generating patch via local model… Streaming response, you can cancel.</span>
          {state.status === "generating" && state.collected ? (
            <pre className="patch-generate__collected" aria-label="Streaming response">
              {state.collected.slice(-500)}
            </pre>
          ) : null}
        </div>
      ) : null}

      <p className="patch-generate__note">Patch uses only selected eligible files (budgeted) + the target file. Original file unchanged until you explicitly apply. Model output is untrusted and validated.</p>
    </div>
  );
}
