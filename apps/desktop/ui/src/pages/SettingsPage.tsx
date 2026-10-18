import { useEffect, useState } from "react";
import { Icon } from "../components/Icon";
import type { AppStatus } from "../lib/ipc";
import { RouterLink } from "../components/RouterLink";
import { getWorkspaceSettings } from "../lib/workspaceSettings";
import { PageHeader } from "./PageHeader";

interface SettingsPageProps {
  status: AppStatus | null;
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
  workspacePath: string | null;
}

const SHORTCUTS: Array<{ keys: string; description: string }> = [
  { keys: "Ctrl + N", description: "Start a new chat" },
  { keys: "Ctrl + L", description: "Focus the message composer" },
  { keys: "Ctrl + B", description: "Collapse or expand the sidebar" },
  { keys: "Esc", description: "Stop the running generation from the composer" },
];

/**
 * Settings: real, persisted interface preferences plus an honest read-only
 * view of engine configuration. No invented controls — engine paths are
 * managed on the Local Models page, not edited here.
 */
export function SettingsPage({
  status,
  sidebarCollapsed,
  onToggleSidebar,
  workspacePath,
}: SettingsPageProps) {
  const [lastWorkspacePath, setLastWorkspacePath] = useState<string | null>(null);

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

  return (
    <div className="page page--settings">
      <PageHeader
        eyebrow="Preferences"
        title="Settings"
        copy="Interface preferences are stored locally with your workspace settings. Model and runtime paths are configured on the Local Models page."
      />

      <div className="page-stack">
        <section className="settings-group" aria-label="Interface">
          <h2 className="settings-group__title">Interface</h2>
          <label className="settings-row">
            <span>
              <span className="settings-row__label">Collapse sidebar</span>
              <span className="settings-row__hint">
                Same as Ctrl + B. The choice is remembered with your workspace
                settings.
              </span>
            </span>
            <input
              type="checkbox"
              checked={sidebarCollapsed}
              onChange={onToggleSidebar}
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
              <Icon name="folder" size={12} /> Projects
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
          </div>
        </section>

        <section className="settings-group" aria-label="Engine configuration">
          <h2 className="settings-group__title">Engine (read-only)</h2>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Runtime</span>
              <span className="settings-row__hint">
                {status?.runtimePath ?? "Not configured"}
              </span>
            </span>
          </div>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Model</span>
              <span className="settings-row__hint">
                {status?.modelPath ?? "Not configured"}
              </span>
            </span>
          </div>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">Context size</span>
              <span className="settings-row__hint">
                {status ? `${status.contextSize} tokens` : "Unknown until the core answers"}
              </span>
            </span>
            <RouterLink to="models" className="button button--ghost button--sm">
              <Icon name="chip" size={12} /> Local models
            </RouterLink>
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

        <section className="settings-group" aria-label="About">
          <h2 className="settings-group__title">About</h2>
          <div className="settings-row settings-row--static">
            <span>
              <span className="settings-row__label">
                {status?.name ?? "Nexora"} {status?.version ?? "0.1.0"}
              </span>
              <span className="settings-row__hint">
                Local-first AI coding studio. Inference, history, and projects
                stay on this machine.
              </span>
            </span>
          </div>
        </section>
      </div>
    </div>
  );
}
