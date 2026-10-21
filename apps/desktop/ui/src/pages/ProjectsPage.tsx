import { EvidencePanel } from "../components/EvidencePanel";
import { ExportPanel } from "../components/ExportPanel";
import { FilePreview } from "../components/FilePreview";
import { Icon } from "../components/Icon";
import { PatchEditor } from "../components/PatchEditor";
import { PatchGenerateBar } from "../components/PatchGenerateBar";
import { PatchPreview } from "../components/PatchPreview";
import { ValidationPanel } from "../components/ValidationPanel";
import { WorkspacePanel } from "../components/WorkspacePanel";
import type { getPatchMissingEvidence } from "../lib/evidence";
import type { AppStatus } from "../lib/ipc";
import type { SymbolIndex } from "../lib/symbolIndex";
import type { usePatch } from "../features/workspace/usePatch";
import type { useValidation } from "../features/workspace/useValidation";
import type { usePatchRepair } from "../features/workspace/usePatchRepair";
import type { useWorkspace } from "../features/workspace/useWorkspace";
import type { buildEvidenceReport } from "../lib/evidence";

type WorkspaceState = ReturnType<typeof useWorkspace>;
type PatchState = ReturnType<typeof usePatch>;
type ValidationState = ReturnType<typeof useValidation>;
type RepairState = ReturnType<typeof usePatchRepair>;
type EvidenceReport = ReturnType<typeof buildEvidenceReport>;
type PatchMissing = ReturnType<typeof getPatchMissingEvidence>;

interface ProjectsPageProps {
  workspace: WorkspaceState;
  patch: PatchState;
  validation: ValidationState;
  repair: RepairState;
  evidenceReport: EvidenceReport | null;
  patchMissing: PatchMissing | null;
  generationTargetPath: string | null;
  generationOriginalContent: string | null;
  groundedContextBlock: string | null;
  engineStatus: AppStatus | null;
  symbolIndex: SymbolIndex | null;
  isStreaming: boolean;
  /** The profile selected in the core; patch requests name it. */
  profileId?: string | null;
}

/**
 * The Projects page: workspace browsing, read-only file preview, and the
 * explicit patch workflow. Chat lives on its own page; this page never embeds
 * the conversation surface.
 */
export function ProjectsPage({
  workspace,
  patch,
  validation,
  repair,
  evidenceReport,
  patchMissing,
  generationTargetPath,
  generationOriginalContent,
  groundedContextBlock,
  engineStatus,
  symbolIndex,
  isStreaming,
  profileId = null,
}: ProjectsPageProps) {
  return (
    <div className="page page--projects">
      <header className="page-header page-header--compact">
        <div>
          <p className="eyebrow">Workspace</p>
          <h1 className="page-header__title">
            {workspace.workspaceName ?? "Projects"}
          </h1>
          <p className="page-header__copy">
            {workspace.workspacePath
              ? workspace.workspacePath
              : "Open a project to browse files, select chat context, and prepare single-file patches. Nothing is modified until you explicitly apply."}
          </p>
        </div>
        {workspace.workspacePath ? (
          <div className="page-header__actions">
            <button
              type="button"
              className="button button--ghost button--sm"
              onClick={() => void workspace.openWorkspace().catch(() => {})}
              disabled={workspace.loading}
              title="Open a different project directory"
            >
              <Icon name="folder" size={13} /> Switch project
            </button>
            <button
              type="button"
              className="button button--ghost button--sm"
              onClick={() => void workspace.refresh().catch(() => {})}
              disabled={workspace.loading}
              title="Rescan project directory for new or changed files"
            >
              <Icon name="refresh" size={13} /> Refresh
            </button>
            <button
              type="button"
              className="button button--ghost button--sm"
              onClick={() => void workspace.closeWorkspace()}
              title="Close the current project"
            >
              <Icon name="x" size={13} /> Close
            </button>
          </div>
        ) : null}
      </header>

      <div className="projects-layout">
        <aside className="workspace__explorer" aria-label="Project explorer">
          <WorkspacePanel
            workspacePath={workspace.workspacePath}
            workspaceName={workspace.workspaceName}
            inventory={workspace.inventory}
            loading={workspace.loading}
            error={workspace.error}
            search={workspace.search}
            onSearchChange={workspace.setSearch}
            filteredEntries={workspace.filteredEntries}
            selectedPaths={workspace.selectedPaths}
            onToggleContext={workspace.toggleContext}
            onClearContext={workspace.clearContext}
            contextError={workspace.contextError}
            onOpen={(p) => void workspace.openWorkspace(p).catch(() => {})}
            onRefresh={() => void workspace.refresh().catch(() => {})}
            onClose={() => void workspace.closeWorkspace()}
            onPreview={workspace.loadPreview}
            previewPath={workspace.previewPath}
            collapsed={false}
          />
        </aside>

        <section className="projects__detail" aria-label="File preview and patch workflow">
          {!workspace.workspacePath ? (
            <div className="empty projects__empty-detail" aria-label="No project open">
              <div className="empty__mark" aria-hidden="true">
                <Icon name="folder" size={36} />
              </div>
              <h2 className="empty__title">No project open</h2>
              <p className="empty__copy">
                Open a project directory to browse files, select read-only context for your AI chats, and generate single-file patches with review-before-apply safety.
              </p>
              <div className="empty__actions">
                <button
                  type="button"
                  className="button button--primary"
                  onClick={() => void workspace.openWorkspace().catch(() => {})}
                  disabled={workspace.loading}
                >
                  <Icon name="folder" size={14} />
                  Open project folder
                </button>
              </div>
              <p className="empty__note">
                Read-only by default: files on disk are never modified without an explicit patch approval.
              </p>
            </div>
          ) : (
            <>
              <FilePreview
                path={workspace.previewPath}
                content={workspace.preview?.content ?? null}
                size={workspace.preview?.size ?? null}
                loading={workspace.previewLoading}
                error={workspace.previewError}
                onClose={workspace.clearPreview}
              />

              <ExportPanel
                workspacePath={workspace.workspacePath}
                selectedPaths={workspace.selectedPaths}
                previewPath={workspace.previewPath}
              />

              {evidenceReport ? (
            <EvidencePanel report={evidenceReport} isStreaming={isStreaming} />
          ) : null}

          {patchMissing && (patchMissing.missing.length > 0 || patchMissing.warnings.length > 0) ? (
            <div className="patch-generate__missing" role="note" aria-label="Patch evidence warning">
              <Icon name="alert" size={12} />
              <div>
                <strong>Required source may be incomplete — patch may be based on partial context.</strong>
                <ul>
                  {patchMissing.warnings.map((w, i) => (
                    <li key={i}>{w}</li>
                  ))}
                  {patchMissing.missing.length > 0 ? (
                    <li>Missing: {patchMissing.missing.join(", ")}</li>
                  ) : null}
                </ul>
                <span className="patch-generate__missing-help">
                  Add the missing files to context or enlarge the budget for a more grounded patch.
                  Never auto-apply — review preview and validation.
                </span>
              </div>
            </div>
          ) : null}

          <PatchGenerateBar
            workspacePath={workspace.workspacePath}
            targetPath={generationTargetPath}
            originalContent={generationOriginalContent}
            filteredContextBlock={groundedContextBlock}
            engineStatus={engineStatus}
            symbolIndex={symbolIndex}
            contextFiles={workspace.contextFiles}
            profileId={profileId}
            onApplied={() => {
              void workspace.refresh();
              if (workspace.previewPath) void workspace.loadPreview(workspace.previewPath);
            }}
            onInvalidate={() => {
              // generation bar handles its own invalidation; manual patch invalidated via workspace switch effect
            }}
          />

          {/* Manual patch workflow — read-only until explicit apply */}
          {workspace.previewPath && workspace.preview?.content !== undefined && !workspace.previewLoading && !workspace.previewError ? (
            <div className="workspace__patch" aria-label="Patch workflow">
              {patch.patchState.status === "idle" ? (
                <button
                  type="button"
                  className="button button--ghost button--sm workspace__propose"
                  disabled={!workspace.workspacePath || workspace.preview?.content?.includes("\0")}
                  onClick={() => {
                    if (workspace.previewPath && workspace.preview?.content !== undefined) {
                      patch.startEditing(workspace.previewPath, workspace.preview!.content);
                    }
                  }}
                  title={!workspace.workspacePath ? "Open a project first" : "Edit a proposal — original file unchanged until you apply"}
                >
                  <Icon name="edit" size={12} /> Propose change
                </button>
              ) : patch.patchState.status === "editing" ? (
                <PatchEditor
                  path={patch.patchState.path}
                  value={patch.editorContent}
                  onChange={patch.updateEditor}
                  onCancel={patch.cancel}
                  onPreview={() => void patch.createProposal()}
                  disabled={patch.editorContent === patch.patchState.originalContent}
                />
              ) : patch.patchState.status === "creating" ? (
                <div className="patch-preview" role="status">
                  <p className="patch-preview__status">Creating proposal… Validating single file, checking traversal, sensitive, binary, size…</p>
                </div>
              ) : patch.patchState.status === "preview" ? (
                <>
                  <PatchPreview
                    proposal={patch.patchState.proposal}
                    validation={patch.patchState.validation}
                    onCancel={patch.cancel}
                    onApply={() => void patch.apply()}
                    applying={false}
                    applied={false}
                  />
                  <ValidationPanel
                    proposal={patch.patchState.proposal}
                    report={validation.validationState.status === "validated" ? validation.validationState.report : null}
                    validating={validation.validationState.status === "validating"}
                    error={validation.validationState.status === "error" ? validation.validationState.message : null}
                    onValidate={() => void validation.validate((patch.patchState as any).proposal)}
                    onCancelValidate={validation.cancel}
                    repairState={repair.repairState as any}
                    onExplain={() => {
                      const rep = validation.validationState.status === "validated" ? validation.validationState.report : null;
                      if (rep) void repair.explain((patch.patchState as any).proposal, rep);
                    }}
                    onRepair={() => {
                      const rep = validation.validationState.status === "validated" ? validation.validationState.report : null;
                      if (rep) void repair.repair((patch.patchState as any).proposal, rep);
                    }}
                    onCancelRepair={repair.cancel}
                    maxAttempts={repair.maxAttempts}
                    revisedProposal={repair.repairState.status === "revised" ? repair.repairState.proposal : null}
                  />
                  {repair.repairState.status === "revised" ? (
                    <div className="workspace__revised" aria-label="Revised patch preview">
                      <h4 className="validation__revised-header">Revised proposal (not yet applied)</h4>
                      <PatchPreview
                        proposal={repair.repairState.proposal}
                        validation={repair.repairState.validation}
                        onCancel={repair.reset}
                        onApply={async () => {
                          // Apply revised via the existing explicit apply path, then refresh.
                          const { applyPatchProposal } = await import("../lib/patch");
                          if (!workspace.workspacePath) return;
                          try {
                            await applyPatchProposal(workspace.workspacePath, (repair.repairState as any).proposal);
                            repair.reset();
                            patch.cancel();
                            void workspace.refresh();
                            if (workspace.previewPath) void workspace.loadPreview(workspace.previewPath);
                          } catch (cause) {
                            // stay on revised preview; the proposal remains unapplied and honest
                            // eslint-disable-next-line no-console
                            console.error(cause);
                          }
                        }}
                        applying={false}
                        applied={false}
                      />
                      <p className="validation__hint">Apply revised explicitly — does not auto-apply. Original proposal remains unchanged above.</p>
                    </div>
                  ) : null}
                </>
              ) : patch.patchState.status === "validating" ? (
                <PatchPreview proposal={patch.patchState.proposal} validation={null} loading onCancel={patch.cancel} onApply={() => {}} />
              ) : patch.patchState.status === "applying" ? (
                <PatchPreview
                  proposal={patch.patchState.proposal}
                  validation={{ ok: true, errors: [], warnings: [], diffSummary: { added: 0, removed: 0, unchanged: 0, originalLines: 0, proposedLines: 0, truncated: false } }}
                  loading={false}
                  applying
                  onCancel={patch.cancel}
                  onApply={() => {}}
                />
              ) : patch.patchState.status === "applied" ? (
                <PatchPreview
                  proposal={patch.patchState.proposal}
                  validation={null}
                  applied
                  onCancel={() => {
                    patch.cancel();
                    void workspace.refresh();
                    if (workspace.previewPath) void workspace.loadPreview(workspace.previewPath);
                  }}
                  onApply={() => {}}
                />
              ) : patch.patchState.status === "error" ? (
                <PatchPreview
                  proposal={patch.patchState.proposal ?? null}
                  validation={null}
                  error={patch.patchState.message}
                  onCancel={patch.cancel}
                  onApply={() => {}}
                />
              ) : null}
              {patch.patchState.status !== "idle" && patch.patchState.status !== "editing" && patch.patchState.status !== "error" ? (
                <p className="workspace__patch-note">Reviewing a diff does not modify the file. Only Apply changes the intended file. Never auto-applies on generate or switch.</p>
              ) : null}
            </div>
          ) : null}
            </>
          )}
        </section>
      </div>
    </div>
  );
}
