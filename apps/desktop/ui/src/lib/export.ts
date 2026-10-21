import { Channel, invoke } from "@tauri-apps/api/core";
import {
  isDesktopShell,
  type ExportError,
  type ExportEvent,
  type ExportOutcome,
  type ExportPlan,
  type SingleExportRequest,
  type ZipExportRequest,
} from "./ipc";

/**
 * Export IPC. Mirrors `src-tauri/src/export/mod.rs`.
 *
 * Everything here is desktop-only: the browser preview has no filesystem to
 * write to, and the UI says so rather than pretending a download happened.
 */

export type {
  ExportCandidate,
  ExportError,
  ExportEvent,
  ExportOutcome,
  ExportPlan,
  ExportProgress,
  SingleExportRequest,
  ZipExportRequest,
} from "./ipc";

const DESKTOP_ONLY: ExportError = {
  code: "desktop_only",
  message:
    "Exports are written by the Nexora desktop app. This browser preview cannot save files.",
};

/** Normalises every transport, serialisation and command failure into one shape. */
export function toExportError(cause: unknown): ExportError {
  if (
    typeof cause === "object" &&
    cause !== null &&
    "code" in cause &&
    "message" in cause &&
    typeof (cause as ExportError).code === "string" &&
    typeof (cause as ExportError).message === "string"
  ) {
    return cause as ExportError;
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
  if (!isDesktopShell()) {
    throw DESKTOP_ONLY;
  }

  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toExportError(cause);
  }
}

/**
 * Lists the files an export would contain. Read-only.
 *
 * `selection` is `null` for the whole project, or an explicit list of
 * workspace-relative paths.
 */
export function planWorkspaceExport(selection: string[] | null): Promise<ExportPlan> {
  return call<ExportPlan>("plan_workspace_export", { selection });
}

/** A non-colliding default destination next to the project folder. */
export function suggestExportPath(
  kind: "zip" | "file",
  relativePath: string | null,
): Promise<string> {
  return call<string>("suggest_export_path", { kind, relativePath });
}

function channel(onEvent: (event: ExportEvent) => void): Channel<ExportEvent> {
  const events = new Channel<ExportEvent>();
  events.onmessage = (event) => onEvent(event);
  return events;
}

/** Copies one workspace file. Only ever called from an explicit user action. */
export async function exportSingleFile(
  request: SingleExportRequest,
  onEvent: (event: ExportEvent) => void,
): Promise<ExportOutcome> {
  if (!isDesktopShell()) {
    throw DESKTOP_ONLY;
  }

  try {
    return await invoke<ExportOutcome>("export_single_file", {
      request,
      onEvent: channel(onEvent),
    });
  } catch (cause) {
    throw toExportError(cause);
  }
}

/** Writes a ZIP of the project, or of an explicit selection. */
export async function exportProjectZip(
  request: ZipExportRequest,
  onEvent: (event: ExportEvent) => void,
): Promise<ExportOutcome> {
  if (!isDesktopShell()) {
    throw DESKTOP_ONLY;
  }

  try {
    return await invoke<ExportOutcome>("export_zip", {
      request,
      onEvent: channel(onEvent),
    });
  } catch (cause) {
    throw toExportError(cause);
  }
}

/** Stops the running export. Returns whether one was active. */
export async function cancelExport(): Promise<boolean> {
  if (!isDesktopShell()) {
    return false;
  }

  try {
    return await invoke<boolean>("cancel_export");
  } catch {
    return false;
  }
}
