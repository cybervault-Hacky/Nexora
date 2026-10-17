import { ApertureMark } from "./ApertureMark";
import { Icon } from "./Icon";
import { isDesktopShell } from "../lib/ipc";
import {
  closeWindow,
  minimizeWindow,
  toggleMaximizeWindow,
} from "../lib/window";

interface TitleBarProps {
  conversationTitle: string;
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
}

/**
 * Custom window chrome.
 *
 * Drag regions are isolated: only the dedicated `data-tauri-drag-region`
 * elements start a window drag, so buttons inside the bar stay clickable.
 * Windows itself handles double-click-to-maximize on a drag region, so no
 * manual handler is attached (doing both toggled the window twice).
 */
export function TitleBar({
  conversationTitle,
  sidebarCollapsed,
  onToggleSidebar,
}: TitleBarProps) {
  const desktop = isDesktopShell();

  return (
    <header className="titlebar">
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

      <div className="titlebar__drag" data-tauri-drag-region>
        <ApertureMark size={16} className="titlebar__mark" />
        <span className="titlebar__wordmark">nexora</span>
        <span className="titlebar__rule" aria-hidden="true" />
        <span className="titlebar__context">{conversationTitle}</span>
      </div>

      <div className="titlebar__spacer" data-tauri-drag-region />

      <div className="titlebar__right">
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
