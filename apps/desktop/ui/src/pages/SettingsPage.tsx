import { useEffect, useState } from "react";
import { Icon } from "../components/Icon";
import type { AppStatus } from "../lib/ipc";
import { RouterLink } from "../components/RouterLink";
import { getWorkspaceSettings, saveWorkspaceSettings } from "../lib/workspaceSettings";
import { useAuth } from "../features/auth/useAuth";
import { PageHeader } from "./PageHeader";

export interface SettingsPageProps {
  status: AppStatus | null;
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
  workspacePath: string | null;
  theme?: "dark" | "light";
  onToggleTheme?: () => void;
  founderModeEnabled?: boolean;
  onToggleFounderMode?: () => void;
  codeLigatures?: boolean;
  onToggleCodeLigatures?: () => void;
  reducedMotion?: boolean;
  onToggleReducedMotion?: () => void;
  onClearLastWorkspace?: () => void;
}

const SHORTCUTS: Array<{ keys: string; description: string }> = [
  { keys: "Ctrl + N", description: "Start a new chat" },
  { keys: "Ctrl + L", description: "Focus the message composer" },
  { keys: "Ctrl + B", description: "Collapse or expand the sidebar" },
  { keys: "Ctrl + ,", description: "Open settings" },
  { keys: "Ctrl + P", description: "Quick open workspace files" },
  { keys: "Esc", description: "Stop the running generation from the composer" },
];

/**
 * Settings: real, persisted interface preferences plus an honest read-only
 * view of engine configuration and local privacy guarantees.
 */
export function SettingsPage({
  status,
  sidebarCollapsed,
  onToggleSidebar,
  workspacePath,
  theme = "dark",
  onToggleTheme,
  founderModeEnabled = false,
  onToggleFounderMode,
  codeLigatures = false,
  onToggleCodeLigatures,
  reducedMotion = false,
  onToggleReducedMotion,
  onClearLastWorkspace,
}: SettingsPageProps) {
  const [lastWorkspacePath, setLastWorkspacePath] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const { user, isAuthenticated } = useAuth();

  useEffect(() => {
    let active = true;
    void getWorkspaceSettings()
      .then((s) => {
        if (active) setLastWorkspacePath(s.lastWorkspacePath ?? null);
      })
      .catch(() => {
        // Settings read is informational; failure is not an error surface.
      });
    return () => {
      active = false;
    };
  }, [workspacePath]);

  const handleClearLastWorkspace = async () => {
    try {
      setLastWorkspacePath(null);
      onClearLastWorkspace?.();
      const current = await getWorkspaceSettings();
      await saveWorkspaceSettings({
        ...current,
        lastWorkspacePath: null,
      });
      setNotice("Cleared remembered workspace path.");
      setTimeout(() => setNotice(null), 3000);
    } catch {
      // ignore
    }
  };

  const engineStateLabel = status?.engine.state
    ? status.engine.state.replace("_", " ")
    : "not configured";

  return (
    <div className="page page--settings" data-testid="settings-page">
      <PageHeader
        eyebrow="Preferences & System"
        title="Settings"
        copy="Interface preferences are stored locally with your workspace settings. Model and runtime paths are configured on the Local Models page."
      />

      {notice && (
        <div className="settings-notice" role="status">
          <Icon name="check" size={14} />
          <span>{notice}</span>
        </div>
      )}

      <div className="page-stack">
        <section className="settings-group" aria-label="Account">
          <h2 className="settings-group__title">Account & Licensing</h2>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">
                {isAuthenticated && user ? user.name || user.email : "Local Guest Mode"}
              </span>
              <span className="settings-row__hint">
                {isAuthenticated && user
                  ? `Signed in as ${user.email} (${user.emailVerified ? "Verified" : "Verification pending"})`
                  : "Unauthenticated. Local LLM chat, workspace, and model tools are 100% functional."}
              </span>
            </span>
            <RouterLink to="account" className="button button--ghost button--sm">
              <Icon name="user" size={12} /> {isAuthenticated ? "Manage account" : "Account details"}
            </RouterLink>
          </div>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Plans & Offline Licensing</span>
              <span className="settings-row__hint">
                Review account-reported plan status and the future-plan concept, or manage an issued offline licence key separately.
              </span>
            </span>
            <div style={{ display: "flex", gap: 8 }}>
              <RouterLink to="plans" className="button button--primary button--sm">
                Plans & Premium
              </RouterLink>
              <RouterLink to="licensing" className="button button--ghost button--sm">
                <Icon name="shield" size={12} /> Offline Licence Keys
              </RouterLink>
            </div>
          </div>
        </section>

        <section className="settings-group" aria-label="Interface">
          <h2 className="settings-group__title">Interface</h2>
          {onToggleTheme && (
            <div className="settings-row settings-row--static">
              <span>
                <span className="settings-row__label">Color theme</span>
                <span className="settings-row__hint">
                  Neutral {theme === "light" ? "Light" : "Dark"} desktop palette active.
                </span>
              </span>
              <button
                type="button"
                className="button button--ghost button--sm"
                onClick={onToggleTheme}
              >
                Switch to {theme === "dark" ? "Light" : "Dark"} theme
              </button>
            </div>
          )}
          <label className="settings-row">
            <span>
              <span className="settings-row__label">Collapse sidebar</span>
              <span className="settings-row__hint">
                Same as Ctrl + B. The choice is remembered with your workspace settings.
              </span>
            </span>
            <input
              type="checkbox"
              aria-label="Collapse sidebar"
              checked={sidebarCollapsed}
              onChange={onToggleSidebar ?? (() => {})}
            />
          </label>
          <label className="settings-row">
            <span>
              <span className="settings-row__label">Code font ligatures</span>
              <span className="settings-row__hint">
                Enable typographic symbol ligatures (e.g. =&gt;, ===) in code blocks and diff previews.
              </span>
            </span>
            <input
              type="checkbox"
              aria-label="Code font ligatures"
              checked={codeLigatures}
              onChange={onToggleCodeLigatures ?? (() => {})}
            />
          </label>
          <label className="settings-row">
            <span>
              <span className="settings-row__label">Reduced motion override</span>
              <span className="settings-row__hint">
                Minimizes interface transitions and decorative animations across the workspace.
              </span>
            </span>
            <input
              type="checkbox"
              aria-label="Reduced motion override"
              checked={reducedMotion}
              onChange={onToggleReducedMotion ?? (() => {})}
            />
          </label>
        </section>

        <section className="settings-group" aria-label="Workspace">
          <h2 className="settings-group__title">Workspace</h2>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Current project</span>
              <span className="settings-row__hint">
                {workspacePath ?? "No project open."}
              </span>
            </span>
            <RouterLink to="projects" className="button button--ghost button--sm">
              <Icon name="folder" size={12} /> Open Projects
            </RouterLink>
          </div>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Last opened path</span>
              <span className="settings-row__hint">
                {lastWorkspacePath ??
                  "Not recorded yet. Nexora never opens a project on its own."}
              </span>
            </span>
            {lastWorkspacePath && (
              <button
                type="button"
                className="button button--ghost button--sm"
                onClick={() => void handleClearLastWorkspace()}
              >
                Clear path
              </button>
            )}
          </div>
        </section>

        <section className="settings-group" aria-label="Privacy & Network">
          <h2 className="settings-group__title">Privacy & Local Sovereignty</h2>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Telemetry & analytics</span>
              <span className="settings-row__hint">
                Disabled by policy. Nexora is 100% offline sovereign; zero metrics, telemetry, or user data leave this machine.
              </span>
            </span>
            <span className="badge badge--local">Offline</span>
          </div>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Local inference boundary</span>
              <span className="settings-row__hint">
                The inference supervisor runs on loopback (127.0.0.1) only, secured with a per-session authentication key.
              </span>
            </span>
            <span className="badge badge--local">127.0.0.1</span>
          </div>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">External AI cloud endpoints</span>
              <span className="settings-row__hint">
                No external APIs, cloud model proxies, or remote inference gateways are contacted.
              </span>
            </span>
            <span className="badge">Zero Cloud</span>
          </div>
        </section>

        <section className="settings-group" aria-label="Engine configuration">
          <h2 className="settings-group__title">Engine & Models (Read-only)</h2>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Engine status</span>
              <span className="settings-row__hint">
                Current state: {engineStateLabel}. {status?.engine.detail ?? ""}
              </span>
            </span>
            <RouterLink to="models" className="button button--primary button--sm">
              <Icon name="chip" size={12} /> Configure local models
            </RouterLink>
          </div>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Runtime binary</span>
              <span className="settings-row__hint">
                {status?.runtimePath ?? "Not configured (requires llama-server b11714 executable)"}
              </span>
            </span>
          </div>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Model file</span>
              <span className="settings-row__hint">
                {status?.modelPath ?? "Not configured (requires GGUF file on local filesystem)"}
              </span>
            </span>
          </div>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Context window</span>
              <span className="settings-row__hint">
                {status ? `${status.contextSize} tokens` : "Unknown until the core answers"}
              </span>
            </span>
          </div>
        </section>

        <section className="settings-group" aria-label="Keyboard shortcuts">
          <h2 className="settings-group__title">Keyboard shortcuts</h2>
          <ul className="settings-shortcuts">
            {SHORTCUTS.map((shortcut) => (
              <li key={shortcut.keys} className="settings-shortcuts__item">
                <kbd className="kbd">{shortcut.keys}</kbd>
                <span>{shortcut.description}</span>
              </li>
            ))}
          </ul>
        </section>

        <section className="settings-group" aria-label="Developer & Founder Tools">
          <h2 className="settings-group__title">Developer & Diagnostics</h2>
          <label className="settings-row">
            <span>
              <span className="settings-row__label">Founder Control Centre</span>
              <span className="settings-row__hint">
                Enables in-memory IPC traffic telemetry, engine diagnostics, and security vault audit view.
              </span>
            </span>
            <input
              type="checkbox"
              aria-label="Founder Control Centre"
              checked={founderModeEnabled}
              onChange={onToggleFounderMode ?? (() => {})}
            />
          </label>
          {founderModeEnabled && (
            <div className="settings-row settings-row--static">
              <span>
                <span className="settings-row__label">Open Founder Console</span>
                <span className="settings-row__hint">
                  Direct access to real-time telemetry and diagnostics.
                </span>
              </span>
              <RouterLink to="founder" className="button button--primary button--sm">
                <Icon name="terminal" size={12} /> Open Console
              </RouterLink>
            </div>
          )}
        </section>

        <section className="settings-group" aria-label="About">
          <h2 className="settings-group__title">About</h2>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">
                {status?.name ?? "Nexora"} {status?.version ?? "0.1.0"}
              </span>
              <span className="settings-row__hint">
                Local-first AI coding studio. Inference, history, and projects
                stay entirely on this machine.
              </span>
            </span>
          </div>
        </section>
      </div>
    </div>
  );
}
