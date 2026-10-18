import { useState } from "react";
import type { WorkspaceInventory } from "../lib/workspace";
import { Icon } from "./Icon";

interface WorkspacePanelProps {
  workspacePath: string | null;
  workspaceName: string | null;
  inventory: WorkspaceInventory | null;
  loading: boolean;
  error: string | null;
  search: string;
  onSearchChange: (value: string) => void;
  filteredEntries: WorkspaceInventory["entries"];
  selectedPaths: Set<string>;
  onToggleContext: (path: string) => void;
  onClearContext: () => void;
  contextError: string | null;
  onOpen: () => void;
  onRefresh: () => void;
  onClose: () => void;
  onPreview: (path: string) => void;
  previewPath: string | null;
  collapsed?: boolean;
}

export function WorkspacePanel({
  workspacePath,
  workspaceName,
  inventory,
  loading,
  error,
  search,
  onSearchChange,
  filteredEntries,
  selectedPaths,
  onToggleContext,
  onClearContext,
  contextError,
  onOpen,
  onRefresh,
  onClose,
  onPreview,
  previewPath,
  collapsed = false,
}: WorkspacePanelProps) {
  const [manualPath, setManualPath] = useState("");

  if (collapsed) return null;

  const hasWorkspace = workspacePath !== null && inventory !== null;

  return (
    <div className="workspace-panel" aria-label="Project workspace">
      <div className="workspace-panel__header">
        <h2 className="sidebar__label">Project</h2>
        {hasWorkspace ? (
          <div className="workspace-panel__actions">
            <button
              type="button"
              className="workspace-panel__icon-btn"
              aria-label="Refresh project"
              title="Refresh"
              onClick={onRefresh}
              disabled={loading}
            >
              <Icon name="refresh" size={13} />
            </button>
            <button
              type="button"
              className="workspace-panel__icon-btn"
              aria-label="Close project"
              title="Close"
              onClick={onClose}
            >
              <Icon name="x" size={13} />
            </button>
          </div>
        ) : null}
      </div>

      {!hasWorkspace ? (
        <div className="workspace-panel__empty">
          <p className="workspace-panel__hint">
            No project selected. Open a local folder to browse files, preview
            code, and attach context to your questions. Nothing leaves this
            machine.
          </p>
          <button type="button" className="workspace-panel__open-btn" onClick={onOpen} disabled={loading}>
            <Icon name="folder" size={14} />
            {loading ? "Opening…" : "Open project"}
          </button>

          <div className="workspace-panel__manual">
            <label className="workspace-panel__manual-label" htmlFor="workspace-manual-path">
              Or type an absolute path
            </label>
            <div className="workspace-panel__manual-row">
              <input
                id="workspace-manual-path"
                className="workspace-panel__manual-input"
                placeholder="C:\Users\you\my-app or /home/you/my-app"
                value={manualPath}
                onChange={(e) => setManualPath(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && manualPath.trim()) {
                    e.preventDefault();
                    // caller handles via onOpen with typed path — we expose via custom event
                    // For simplicity, trigger prompt fallback; parent can read manualPath via closure if needed
                    const ev = new CustomEvent("workspace:openManual", { detail: manualPath.trim() });
                    window.dispatchEvent(ev);
                  }
                }}
              />
              <button
                type="button"
                className="workspace-panel__manual-btn"
                disabled={!manualPath.trim() || loading}
                onClick={() => {
                  const ev = new CustomEvent("workspace:openManual", { detail: manualPath.trim() });
                  window.dispatchEvent(ev);
                }}
              >
                Open
              </button>
            </div>
          </div>

          {error ? (
            <div className="workspace-panel__error" role="alert">
              <Icon name="alert" size={12} />
              <span>{error}</span>
            </div>
          ) : null}
          <p className="workspace-panel__note">
            In the desktop app, a native folder picker is used. In browser
            preview, type a path to test validation.
          </p>
        </div>
      ) : (
        <>
          <div className="workspace-panel__meta">
            <div className="workspace-panel__name" title={workspaceName ?? ""}>
              <Icon name="folder" size={13} />
              <span>{workspaceName}</span>
            </div>
            <div className="workspace-panel__path" title={workspacePath ?? ""}>
              {workspacePath}
            </div>
            <div className="workspace-panel__switch-row">
              <button type="button" className="workspace-panel__link-btn" onClick={onOpen}>
                Switch project
              </button>
              <span className="workspace-panel__dot" aria-hidden="true">
                ·
              </span>
              <button type="button" className="workspace-panel__link-btn" onClick={onClearContext} title="Clear selected context files">
                Clear context
              </button>
            </div>
          </div>

          {error ? (
            <div className="workspace-panel__error" role="alert">
              <Icon name="alert" size={12} />
              <span>{error}</span>
              <button type="button" className="history__retry" onClick={onRefresh}>
                Retry
              </button>
            </div>
          ) : null}

          {contextError ? (
            <div className="workspace-panel__error" role="alert">
              <Icon name="alert" size={12} />
              <span>{contextError}</span>
            </div>
          ) : null}

          <div className="workspace-panel__search">
            <Icon name="search" size={13} />
            <input
              className="workspace-panel__search-input"
              placeholder="Search files…"
              aria-label="Search files"
              value={search}
              onChange={(e) => onSearchChange(e.target.value)}
            />
            {search ? (
              <button
                type="button"
                className="workspace-panel__search-clear"
                aria-label="Clear search"
                onClick={() => onSearchChange("")}
              >
                <Icon name="x" size={12} />
              </button>
            ) : null}
          </div>

          <div className="workspace-panel__stats" aria-live="polite">
            <span>
              {filteredEntries.length} of {inventory?.entries.length ?? 0} items
              {inventory?.truncated ? " — truncated" : ""}
            </span>
            {inventory?.stats ? (
              <span className="workspace-panel__stats-detail">
                {inventory.stats.fileCount} files, {inventory.stats.dirCount} dirs
                {inventory.stats.skippedExcluded > 0 ? `, ${inventory.stats.skippedExcluded} excluded` : ""}
                {inventory.stats.skippedSymlinkOutside > 0 ? `, ${inventory.stats.skippedSymlinkOutside} symlinks skipped` : ""}
                {inventory.stats.skippedDepth > 0 ? `, ${inventory.stats.skippedDepth} depth-limited` : ""}
              </span>
            ) : null}
          </div>

          {loading ? (
            <p className="workspace-panel__loading" role="status">
              Scanning project…
            </p>
          ) : filteredEntries.length === 0 ? (
            <p className="workspace-panel__empty-text">
              {search ? `No files match “${search}”.` : "No files to display. This folder may be empty or contain only excluded items."}
            </p>
          ) : (
            <ul className="workspace-panel__list" role="list" aria-label="Project files">
              {filteredEntries.map((entry) => {
                const isDir = entry.kind === "dir";
                const isSelected = selectedPaths.has(entry.path);
                const isPreviewed = previewPath === entry.path;
                return (
                  <li
                    key={entry.path}
                    className={`workspace-panel__item ${isDir ? "workspace-panel__item--dir" : ""} ${isPreviewed ? "workspace-panel__item--previewed" : ""}`}
                    role="listitem"
                  >
                    <div className="workspace-panel__item-main">
                      <button
                        type="button"
                        className="workspace-panel__item-open"
                        title={entry.path}
                        onClick={() => !isDir && onPreview(entry.path)}
                        disabled={isDir}
                        aria-label={isDir ? `${entry.path} directory` : `Preview ${entry.path}`}
                      >
                        <span className="workspace-panel__item-icon" aria-hidden="true">
                          <Icon name={isDir ? "folder" : "file"} size={12} />
                        </span>
                        <span className="workspace-panel__item-path">{entry.path}</span>
                        {!isDir ? <span className="workspace-panel__item-size">{formatSize(entry.size)}</span> : null}
                      </button>
                    </div>
                    {!isDir ? (
                      <label className="workspace-panel__item-check" title="Include as context (read-only)">
                        <input
                          type="checkbox"
                          checked={isSelected}
                          onChange={() => onToggleContext(entry.path)}
                          aria-label={`Include ${entry.path} as context`}
                        />
                        <span className="workspace-panel__item-check-label">Context</span>
                      </label>
                    ) : null}
                  </li>
                );
              })}
            </ul>
          )}

          {inventory?.truncated ? (
            <p className="workspace-panel__truncated" role="note">
              Results truncated at {inventory.limits.maxFiles} files / depth {inventory.limits.maxDepth}. Use search or narrow the folder.
            </p>
          ) : null}
        </>
      )}
    </div>
  );
}

function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}
