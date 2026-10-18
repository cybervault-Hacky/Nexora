import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell } from "./ipc";

export interface WorkspaceEntry {
  path: string;
  kind: "file" | "dir";
  size: number;
  depth: number;
}

export interface WorkspaceStats {
  fileCount: number;
  dirCount: number;
  skippedExcluded: number;
  skippedSensitive: number;
  skippedOversized: number;
  skippedUnreadable: number;
  skippedSymlinkOutside: number;
  skippedDepth: number;
}

export interface WorkspaceLimits {
  maxDepth: number;
  maxFiles: number;
  maxPreviewBytes: number;
  maxInventoryBytes: number;
}

export interface WorkspaceInventory {
  rootName: string;
  rootPath: string;
  entries: WorkspaceEntry[];
  truncated: boolean;
  stats: WorkspaceStats;
  limits: WorkspaceLimits;
}

export interface FilePreview {
  path: string;
  content: string;
  size: number;
  truncated: boolean;
  encoding: string;
}

export interface WorkspaceError {
  code: string;
  message: string;
}

function toWorkspaceError(cause: unknown): WorkspaceError {
  if (
    typeof cause === "object" &&
    cause !== null &&
    "code" in cause &&
    "message" in cause &&
    typeof (cause as WorkspaceError).code === "string" &&
    typeof (cause as WorkspaceError).message === "string"
  ) {
    return cause as WorkspaceError;
  }
  if (typeof cause === "string" && cause.trim().length > 0) {
    return { code: "ipc", message: cause };
  }
  if (cause instanceof Error && cause.message.trim().length > 0) {
    return { code: "ipc", message: cause.message };
  }
  return { code: "ipc", message: "The desktop core did not answer." };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toWorkspaceError(cause);
  }
}

const STORAGE_KEY = "nexora:workspace:v1";

// Frontend persistence for convenience (local path, not secret)
export function loadStoredWorkspace(): string | null {
  try {
    if (typeof localStorage === "undefined") return null;
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { path?: string };
    if (typeof parsed.path === "string" && parsed.path.trim()) return parsed.path;
    return null;
  } catch {
    return null;
  }
}

export function storeWorkspace(path: string | null): void {
  try {
    if (typeof localStorage === "undefined") return;
    if (path) localStorage.setItem(STORAGE_KEY, JSON.stringify({ path }));
    else localStorage.removeItem(STORAGE_KEY);
  } catch {
    // ignore quota
  }
}

export function clearStoredWorkspace(): void {
  storeWorkspace(null);
}

export async function pickWorkspaceDirectory(): Promise<string | null> {
  // Try native dialog via Tauri plugin if available
  if (isDesktopShell()) {
    try {
      // Dynamic import so tests/browser without plugin still work — constructed
      // string prevents Vite from trying to resolve the optional plugin at build time.
      // eslint-disable-next-line @typescript-eslint/ban-ts-comment
      // @ts-ignore — optional dependency, may not be installed in browser preview
      const spec = "@tauri-apps" + "/plugin-dialog";
      const mod = await import(/* @vite-ignore */ spec);
      const open = (mod as unknown as { open: (opts: unknown) => Promise<string | string[] | null> }).open;
      if (typeof open === "function") {
        const selected = await open({ directory: true, multiple: false });
        if (typeof selected === "string" && selected.trim()) return selected;
        if (Array.isArray(selected) && selected.length > 0 && typeof selected[0] === "string") return selected[0] as string;
        return null; // cancelled
      }
    } catch {
      // plugin not available — fall through to Rust command or prompt
    }
    // Fallback: ask Rust to pick (will error with guidance, but we try)
    try {
      const picked = await call<string | null>("pick_workspace_dialog");
      if (typeof picked === "string" && picked.trim()) return picked;
      return null;
    } catch {
      // ignore
    }
  }
  // Browser/preview fallback: manual entry
  if (typeof window !== "undefined" && typeof window.prompt === "function") {
    const manual = window.prompt("Enter project folder path (absolute):");
    if (manual && manual.trim()) return manual.trim();
  }
  return null;
}

export async function setWorkspace(path: string): Promise<WorkspaceInventory> {
  if (!isDesktopShell()) {
    throw toWorkspaceError({ code: "desktop_only", message: "Workspace browsing is only available in the desktop app." });
  }
  const inv = await call<WorkspaceInventory>("set_workspace", { path });
  storeWorkspace(inv.rootPath);
  return inv;
}

export async function scanWorkspace(root: string): Promise<WorkspaceInventory> {
  if (!isDesktopShell()) {
    throw toWorkspaceError({ code: "desktop_only", message: "Workspace browsing is only available in the desktop app." });
  }
  return call<WorkspaceInventory>("scan_workspace_command", { root });
}

export async function scanCurrentWorkspace(): Promise<WorkspaceInventory> {
  if (!isDesktopShell()) {
    throw toWorkspaceError({ code: "desktop_only", message: "Workspace browsing is only available in the desktop app." });
  }
  return call<WorkspaceInventory>("scan_current_workspace");
}

export async function readWorkspaceFile(root: string, relativePath: string): Promise<FilePreview> {
  if (!isDesktopShell()) {
    throw toWorkspaceError({ code: "desktop_only", message: "File preview is only available in the desktop app." });
  }
  return call<FilePreview>("read_workspace_file_command", { root, relative_path: relativePath });
}

export async function getWorkspaceContext(root: string, paths: string[]): Promise<FilePreview[]> {
  if (!isDesktopShell()) {
    throw toWorkspaceError({ code: "desktop_only", message: "Workspace context is only available in the desktop app." });
  }
  return call<FilePreview[]>("get_workspace_context", { root, paths });
}

export async function closeWorkspace(): Promise<boolean> {
  if (!isDesktopShell()) {
    clearStoredWorkspace();
    return true;
  }
  try {
    const res = await call<boolean>("close_workspace");
    clearStoredWorkspace();
    return res;
  } catch (cause) {
    throw toWorkspaceError(cause);
  }
}

export async function getWorkspace(): Promise<string | null> {
  if (!isDesktopShell()) return loadStoredWorkspace();
  try {
    const res = await call<string | null>("get_workspace");
    if (res) storeWorkspace(res);
    return res;
  } catch {
    return loadStoredWorkspace();
  }
}

// Utility for tests
export function __resetWorkspaceStorageForTest(): void {
  clearStoredWorkspace();
}
