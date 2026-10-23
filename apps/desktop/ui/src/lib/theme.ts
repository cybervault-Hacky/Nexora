export type ThemeMode = "dark" | "light";

export const THEME_STORAGE_KEY = "nexora.theme";

/**
 * Reads the persisted theme preference without throwing in restricted WebViews.
 * Defaults to ChatGPT neutral dark mode when no preference has been stored.
 */
export function readPersistedTheme(): ThemeMode {
  if (typeof window === "undefined") {
    return "dark";
  }
  try {
    const saved = window.localStorage.getItem(THEME_STORAGE_KEY);
    if (saved === "light" || saved === "dark") {
      return saved;
    }
  } catch {
    // Ignore storage access errors in restricted sandboxes.
  }
  const attr =
    typeof document !== "undefined" ? document.documentElement.dataset.theme : undefined;
  return attr === "light" ? "light" : "dark";
}

export function applyThemePreference(theme: ThemeMode): void {
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = theme;
  }
  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(THEME_STORAGE_KEY, theme);
    } catch {
      // Ignore storage access errors in restricted sandboxes.
    }
  }
}
