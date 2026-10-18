import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell } from "./ipc";

export interface WorkspaceSettings {
  version: number;
  lastWorkspacePath?: string | null;
  preferences: {
    explorerCollapsed: boolean;
  };
}

function toError(cause: unknown): { code: string; message: string } {
  if (typeof cause === "object" && cause !== null && "code" in cause && "message" in cause) {
    return cause as { code: string; message: string };
  }
  return { code: "ipc", message: cause instanceof Error ? cause.message : String(cause) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toError(cause);
  }
}

const FALLBACK_KEY = "nexora:workspace-settings:v1";

function fallbackLoad(): WorkspaceSettings {
  try {
    if (typeof localStorage === "undefined") return { version: 1, lastWorkspacePath: null, preferences: { explorerCollapsed: false } };
    const raw = localStorage.getItem(FALLBACK_KEY);
    if (!raw) return { version: 1, lastWorkspacePath: null, preferences: { explorerCollapsed: false } };
    const parsed = JSON.parse(raw) as WorkspaceSettings;
    if (!parsed || typeof parsed.version !== "number" || parsed.version !== 1) return { version: 1, lastWorkspacePath: null, preferences: { explorerCollapsed: false } };
    return {
      version: 1,
      lastWorkspacePath: typeof parsed.lastWorkspacePath === "string" ? parsed.lastWorkspacePath : null,
      preferences: { explorerCollapsed: Boolean(parsed.preferences?.explorerCollapsed) },
    };
  } catch {
    return { version: 1, lastWorkspacePath: null, preferences: { explorerCollapsed: false } };
  }
}

function fallbackSave(settings: WorkspaceSettings): void {
  try {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(FALLBACK_KEY, JSON.stringify(settings));
  } catch {
    // ignore
  }
}

export async function getWorkspaceSettings(): Promise<WorkspaceSettings> {
  if (!isDesktopShell()) return fallbackLoad();
  try {
    return await call<WorkspaceSettings>("get_workspace_settings");
  } catch {
    return fallbackLoad();
  }
}

export async function saveWorkspaceSettings(settings: WorkspaceSettings): Promise<WorkspaceSettings> {
  if (!isDesktopShell()) {
    fallbackSave(settings);
    return settings;
  }
  try {
    const saved = await call<WorkspaceSettings>("save_workspace_settings", { settings });
    fallbackSave(saved);
    return saved;
  } catch {
    fallbackSave(settings);
    return settings;
  }
}
