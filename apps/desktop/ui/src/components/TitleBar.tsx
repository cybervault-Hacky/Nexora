import { useEffect, useState } from "react";
import { ApertureMark } from "./ApertureMark";
import { Icon } from "./Icon";
import { isDesktopShell } from "../lib/ipc";
import { applyThemePreference, readPersistedTheme, type ThemeMode } from "../lib/theme";
import {
  closeWindow,
  minimizeWindow,
  toggleMaximizeWindow,
} from "../lib/window";

interface TitleBarProps {
  /** Name of the active page — shown as the window's context, not the conversation title. */
  pageLabel: string;
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
  showSidebarToggle?: boolean;
}

/**
 * Custom desktop window chrome styled in neutral ChatGPT charcoal/light tones.
 */
export function TitleBar({
  pageLabel,
  sidebarCollapsed,
  onToggleSidebar,
  showSidebarToggle = true,
}: TitleBarProps) {
  const desktop = isDesktopShell();
  const [theme, setTheme] = useState<ThemeMode>(() => readPersistedTheme());

  useEffect(() => {
    applyThemePreference(theme);
  }, [theme]);

  const toggleTheme = () => {
    setTheme((prev) => (prev === "dark" ? "light" : "dark"));
  };

  return (
    <header className="titlebar">
      {showSidebarToggle ? (
        <button
          type="button"
          className="titlebar__toggle"
          aria-label={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-expanded={!sidebarCollapsed}
          aria-controls="sidebar"
          aria-keyshortcuts="Control+B"
          title={`${sidebarCollapsed ? "Expand" : "Collapse"} sidebar (Ctrl+B)`}
          onClick={onToggleSidebar}
        >
          <Icon name="panel-left" size={15} />
        </button>
      ) : (
        <span className="titlebar__toggle titlebar__toggle--hidden" aria-hidden="true" />
      )}

      <div className="titlebar__drag" data-tauri-drag-region="true">
        <ApertureMark size={16} className="titlebar__mark" />
        <span className="titlebar__wordmark">Nexora</span>
        <span className="titlebar__rule" aria-hidden="true" />
        <span className="titlebar__context">{pageLabel}</span>
      </div>

      <div className="titlebar__spacer" data-tauri-drag-region="true" />

      <div className="titlebar__right">
        <button
          type="button"
          className="titlebar__toggle"
          onClick={toggleTheme}
          aria-label={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}
          title={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}
        >
          <Icon name={theme === "dark" ? "sun" : "moon"} size={15} />
        </button>

        <span
          className="badge badge--local"
          title="No cloud service is used for inference"
        >
          <span className="badge__dot" aria-hidden="true" />
          Local
        </span>

        {desktop ? (
          <div className="window-controls">
            <button
              type="button"
              className="window-control"
              aria-label="Minimize window"
              title="Minimize"
              onClick={() => void minimizeWindow()}
            >
              <Icon name="minimize" size={14} />
            </button>
            <button
              type="button"
              className="window-control"
              aria-label="Maximize or restore window"
              title="Maximize or restore"
              onClick={() => void toggleMaximizeWindow()}
            >
              <Icon name="maximize" size={12} />
            </button>
            <button
              type="button"
              className="window-control window-control--close"
              aria-label="Close window"
              title="Close"
              onClick={() => void closeWindow()}
            >
              <Icon name="close" size={14} />
            </button>
          </div>
        ) : null}
      </div>
    </header>
  );
}
