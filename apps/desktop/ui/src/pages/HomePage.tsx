import { useEffect, useState, type FormEvent } from "react";
import type { AppStatus } from "../lib/ipc";
import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";
import { navigateTo } from "../lib/router";
import { getWorkspaceSettings } from "../lib/workspaceSettings";
import { PageHeader } from "./PageHeader";

export interface RecentConversationItem {
  id: string;
  title: string;
  updatedAt: number;
}

export interface HomePageProps {
  status: AppStatus | null;
  statusError: string | null;
  engineReady: boolean;
  workspacePath: string | null;
  workspaceName: string | null;
  fileCount?: number;
  selectedFileCount?: number;
  symbolCount?: number;
  relationshipCount?: number;
  recentConversations?: RecentConversationItem[];
  conversationCount: number;
  onNewChat: () => void;
  onOpenProject?: () => void;
  onOpenPath?: (path: string) => Promise<void> | void;
  onSelectConversation?: (id: string) => void;
}

function engineLabel(
  status: AppStatus | null,
  statusError: string | null,
): string {
  if (statusError) return "Status unknown";
  switch (status?.engine.state) {
    case "ready":
      return "Ready";
    case "generating":
      return "Generating";
    case "loading":
      return "Loading";
    case "error":
      return "Failed";
    case "stopped":
      return "Stopped";
    case "not_configured":
      return "No model loaded";
    default:
      return "Checking…";
  }
}

function formatTimestamp(ts: number): string {
  if (!ts) return "";
  const now = Date.now();
  const diffMinutes = Math.floor((now - ts) / 60000);
  if (diffMinutes < 1) return "Just now";
  if (diffMinutes < 60) return `${diffMinutes}m ago`;
  const diffHours = Math.floor(diffMinutes / 60);
  if (diffHours < 24) return `${diffHours}h ago`;
  return new Date(ts).toLocaleDateString();
}

function extractModelFileName(fullPath: string | null | undefined): string {
  if (!fullPath) return "None loaded";
  const normalized = fullPath.replace(/\\/g, "/");
  const parts = normalized.split("/");
  return parts[parts.length - 1] || fullPath;
}

/**
 * Phase 24 — Dashboard / Home Central Workspace
 *
 * Serves as the central operational hub for the Nexora desktop software:
 * - Direct quick actions to all primary product areas.
 * - Project management: open folder, resume remembered recent projects, manual path open.
 * - Genuine AI engine readiness state: supervisor lifecycle, active GGUF model, context limit.
 * - Code intelligence overview: cataloged symbols and dependency graph connections.
 * - Recent conversation activity with 1-click continuation.
 * - Honest offline sovereignty guarantees without any mandatory cloud connections.
 */
export function HomePage({
  status,
  statusError,
  engineReady,
  workspacePath,
  workspaceName,
  fileCount = 0,
  selectedFileCount = 0,
  symbolCount = 0,
  relationshipCount = 0,
  recentConversations = [],
  conversationCount,
  onNewChat,
  onOpenProject,
  onOpenPath,
  onSelectConversation,
}: HomePageProps) {
  const [rememberedPath, setRememberedPath] = useState<string | null>(null);
  const [manualPathInput, setManualPathInput] = useState("");
  const [openingPath, setOpeningPath] = useState(false);

  useEffect(() => {
    let active = true;
    void getWorkspaceSettings()
      .then((settings) => {
        if (active && settings.lastWorkspacePath) {
          setRememberedPath(settings.lastWorkspacePath);
        }
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [workspacePath]);

  const handleManualOpen = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const target = manualPathInput.trim();
    if (!target || !onOpenPath) return;

    setOpeningPath(true);
    try {
      await onOpenPath(target);
      setManualPathInput("");
    } finally {
      setOpeningPath(false);
    }
  };

  const handleResumeRecent = async () => {
    if (!rememberedPath || !onOpenPath) return;
    setOpeningPath(true);
    try {
      await onOpenPath(rememberedPath);
    } finally {
      setOpeningPath(false);
    }
  };

  const handleSelectRecent = (id: string) => {
    if (onSelectConversation) {
      onSelectConversation(id);
    }
    navigateTo("chat");
  };

  return (
    <div className="page page--home">
      <PageHeader
        eyebrow="Local-first AI coding studio"
        title="Welcome to Nexora"
        copy="Plan, write, and check code with a model that runs on this machine. Pick a workspace below, or jump straight into a conversation."
      />

      {/* Primary Quick Actions */}
      <section className="home-actions" aria-label="Quick actions">
        <button type="button" className="home-action home-action--primary" onClick={onNewChat}>
          <Icon name="message" size={16} />
          <span className="home-action__title">Start a new chat</span>
          <span className="home-action__hint">Open the AI Chat page with an empty conversation</span>
        </button>
        <RouterLink to="projects" className="home-action">
          <Icon name="folder" size={16} />
          <span className="home-action__title">Projects</span>
          <span className="home-action__hint">Open a folder, browse files, and prepare patches</span>
        </RouterLink>
        <RouterLink to="code" className="home-action">
          <Icon name="code" size={16} />
          <span className="home-action__title">Code intelligence</span>
          <span className="home-action__hint">Search indexed symbols and inspect the dependency graph</span>
        </RouterLink>
        <RouterLink to="models" className="home-action">
          <Icon name="chip" size={16} />
          <span className="home-action__title">Local models</span>
          <span className="home-action__hint">Connect a llama.cpp runtime and a GGUF model you already have</span>
        </RouterLink>
      </section>

      {/* Central Dashboard Status Cards */}
      <section className="home-cards" aria-label="Application status">
        {/* Card 1: AI Engine Readiness */}
        <div className="home-card">
          <h2 className="home-card__title">
            <span
              className={`status-dot status-dot--sm ${engineReady ? "status-dot--ready" : ""}`}
              aria-hidden="true"
            />
            AI engine
          </h2>
          <p className="home-card__value">{engineLabel(status, statusError)}</p>
          <p className="home-card__detail">
            {statusError
              ? "The desktop core did not answer; the engine state cannot be confirmed."
              : status?.modelPath
                ? `Active model: ${extractModelFileName(status.modelPath)} (${status.contextSize} tokens)`
                : (status?.engine.detail ?? "Reading engine status…")}
          </p>
          {!engineReady ? (
            <RouterLink to="models" className="home-card__link">
              Open local models
            </RouterLink>
          ) : (
            <RouterLink to="models" className="home-card__link">
              Model settings
            </RouterLink>
          )}
        </div>

        {/* Card 2: Workspace & Codebase */}
        <div className="home-card">
          <h2 className="home-card__title">
            <Icon name="folder" size={13} />
            Project
          </h2>
          <p className="home-card__value">
            {workspaceName ?? (workspacePath ? "Open" : "No project open")}
          </p>
          <p className="home-card__detail">
            {workspacePath ? (
              <>
                <span style={{ display: "block", marginBottom: 4, fontFamily: "monospace", fontSize: 11 }}>
                  {workspacePath}
                </span>
                <span>
                  {fileCount} {fileCount === 1 ? "file" : "files"} in inventory
                  {selectedFileCount > 0 ? ` · ${selectedFileCount} selected for context` : ""}
                </span>
              </>
            ) : (
              "Open a local directory to browse files, select context, and prepare code patches."
            )}
          </p>

          <div style={{ display: "flex", flexWrap: "wrap", gap: 8, marginTop: 4 }}>
            {workspacePath ? (
              <RouterLink to="projects" className="home-card__link">
                Manage project
              </RouterLink>
            ) : (
              <>
                {onOpenProject && (
                  <button
                    type="button"
                    className="button button--primary button--sm"
                    onClick={onOpenProject}
                    style={{ fontSize: 11 }}
                  >
                    Open folder…
                  </button>
                )}
                {rememberedPath && onOpenPath && (
                  <button
                    type="button"
                    className="button button--ghost button--sm"
                    onClick={handleResumeRecent}
                    disabled={openingPath}
                    style={{ fontSize: 11 }}
                    title={rememberedPath}
                  >
                    {openingPath ? "Opening…" : "Reopen recent"}
                  </button>
                )}
                <RouterLink to="projects" className="home-card__link">
                  Browse Projects
                </RouterLink>
              </>
            )}
          </div>
        </div>

        {/* Card 3: Saved Conversations & Activity */}
        <div className="home-card">
          <h2 className="home-card__title">
            <Icon name="message" size={13} />
            Conversations
          </h2>
          <p className="home-card__value">
            {conversationCount === 1 ? "1 saved" : `${conversationCount} saved`}
          </p>
          <div className="home-card__detail">
            {recentConversations.length > 0 ? (
              <ul style={{ listStyle: "none", padding: 0, margin: "4px 0", display: "flex", flexDirection: "column", gap: 6 }}>
                {recentConversations.slice(0, 3).map((chat) => (
                  <li key={chat.id} style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <button
                      type="button"
                      className="text-action"
                      style={{
                        textAlign: "left",
                        padding: 0,
                        fontSize: 12,
                        maxWidth: 180,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                        color: "var(--accent, #a78bfa)",
                        background: "none",
                        border: "none",
                        cursor: "pointer",
                      }}
                      onClick={() => handleSelectRecent(chat.id)}
                      title={`Open: ${chat.title}`}
                    >
                      {chat.title}
                    </button>
                    <span style={{ fontSize: 10, color: "var(--text-tertiary)" }}>
                      {formatTimestamp(chat.updatedAt)}
                    </span>
                  </li>
                ))}
              </ul>
            ) : (
              "History is stored locally on this machine. Continue any conversation from the sidebar."
            )}
          </div>
          <RouterLink to="chat" className="home-card__link">
            Go to AI Chat
          </RouterLink>
        </div>

        {/* Card 4: Code Intelligence & Symbol Graph */}
        <div className="home-card">
          <h2 className="home-card__title">
            <Icon name="code" size={13} />
            Code intelligence
          </h2>
          <p className="home-card__value">
            {symbolCount > 0
              ? `${symbolCount} symbols`
              : workspacePath
                ? "Index ready"
                : "Not indexed"}
          </p>
          <p className="home-card__detail">
            {symbolCount > 0 ? (
              <span>
                {symbolCount} symbol definitions cataloged
                {relationshipCount > 0 ? ` · ${relationshipCount} dependency edges mapped` : ""}
              </span>
            ) : workspacePath ? (
              "Open Code Intelligence to index AST symbols and inspect file relationships."
            ) : (
              "Symbols and dependency relationships are mapped directly from your local files without external servers."
            )}
          </p>
          <RouterLink to="code" className="home-card__link">
            {symbolCount > 0 ? "Inspect symbol index" : "Explore code intelligence"}
          </RouterLink>
        </div>
      </section>

      {/* Manual Open Bar for Workspace Folders */}
      {!workspacePath && onOpenPath && (
        <section
          className="settings-group"
          style={{ maxWidth: 980, width: "100%", marginTop: 16 }}
          aria-label="Direct workspace path"
        >
          <h2 className="settings-group__title">Open Workspace Directory</h2>
          <form
            onSubmit={handleManualOpen}
            style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}
          >
            <input
              type="text"
              className="input"
              style={{ flex: "1 1 320px", fontFamily: "monospace", fontSize: 12 }}
              placeholder={rememberedPath ? `e.g. ${rememberedPath}` : "Enter absolute directory path (e.g. C:\\Projects\\MyApp)"}
              value={manualPathInput}
              onChange={(e) => setManualPathInput(e.target.value)}
              disabled={openingPath}
            />
            <button
              type="submit"
              className="button button--primary button--sm"
              disabled={openingPath || !manualPathInput.trim()}
            >
              {openingPath ? "Opening…" : "Open path"}
            </button>
          </form>
        </section>
      )}
    </div>
  );
}
