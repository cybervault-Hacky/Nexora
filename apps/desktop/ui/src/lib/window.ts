import { getCurrentWindow } from "@tauri-apps/api/window";
import { isDesktopShell } from "./ipc";

async function withWindow(action: (window: ReturnType<typeof getCurrentWindow>) => Promise<void>) {
  if (!isDesktopShell()) {
    return;
  }

  try {
    await action(getCurrentWindow());
  } catch (error) {
    // Window controls are cosmetic; never surface a raw platform error.
    console.warn("Window action failed", error);
  }
}

export function minimizeWindow(): Promise<void> {
  return withWindow((window) => window.minimize());
}

export function toggleMaximizeWindow(): Promise<void> {
  return withWindow((window) => window.toggleMaximize());
}

export function closeWindow(): Promise<void> {
  return withWindow((window) => window.close());
}
