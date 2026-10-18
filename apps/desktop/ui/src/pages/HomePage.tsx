import type { AppStatus } from "../lib/ipc";
import { Icon } from "../components/Icon";
import { RouterLink } from "../components/RouterLink";
import { PageHeader } from "./PageHeader";

interface HomePageProps {
  status: AppStatus | null;
  statusError: string | null;
  engineReady: boolean;
  workspacePath: string | null;
  workspaceName: string | null;
  conversationCount: number;
  onNewChat: () => void;
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

/**
 * Home / dashboard: an honest overview and real navigation only.
 * It never embeds the chat transcript or the projects interface.
 */
export function HomePage({
  status,
  statusError,
  engineReady,
  workspacePath,
  workspaceName,
  conversationCount,
  onNewChat,
}: HomePageProps) {
  return (
    <div className="page page--home">
      <PageHeader
        eyebrow="Local-first AI coding studio"
        title="Welcome to Nexora"
        copy="Plan, write, and check code with a model that runs on this machine. Pick a workspace below, or jump straight into a conversation."
      />

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

      <section className="home-cards" aria-label="Application status">
        <div className="home-card">
          <h2 className="home-card__title">
            <span className="status-dot status-dot--sm" aria-hidden="true" />
            AI engine
          </h2>
          <p className="home-card__value">{engineLabel(status, statusError)}</p>
          <p className="home-card__detail">
            {statusError
              ? "The desktop core did not answer; the engine state cannot be confirmed."
              : (status?.engine.detail ?? "Reading engine status…")}
          </p>
          {!engineReady ? (
            <RouterLink to="models" className="home-card__link">
              Open local models
            </RouterLink>
          ) : null}
        </div>

        <div className="home-card">
          <h2 className="home-card__title">
            <Icon name="folder" size={13} />
            Project
          </h2>
          <p className="home-card__value">
            {workspaceName ?? (workspacePath ? "Open" : "No project open")}
          </p>
          <p className="home-card__detail">
            {workspacePath
              ? workspacePath
              : "Open a project to browse files, select chat context, and prepare patches."}
          </p>
          <RouterLink to="projects" className="home-card__link">
            {workspacePath ? "Manage project" : "Open a project"}
          </RouterLink>
        </div>

        <div className="home-card">
          <h2 className="home-card__title">
            <Icon name="message" size={13} />
            Conversations
          </h2>
          <p className="home-card__value">
            {conversationCount === 1 ? "1 saved" : `${conversationCount} saved`}
          </p>
          <p className="home-card__detail">
            History is stored locally on this machine. Continue any conversation from the sidebar.
          </p>
          <RouterLink to="chat" className="home-card__link">
            Go to AI Chat
          </RouterLink>
        </div>
      </section>
    </div>
  );
}
