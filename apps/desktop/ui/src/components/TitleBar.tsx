import { Icon } from "./Icon";
import { isDesktopShell } from "../lib/ipc";
import {
  closeWindow,
  minimizeWindow,
  toggleMaximizeWindow,
} from "../lib/window";

interface TitleBarProps {
  /** Name of the active page for assistive window-chrome context. */
  pageLabel: string;
}

/**
 * Custom desktop window chrome styled in neutral ChatGPT charcoal/light tones.
 */
export function TitleBar({
  pageLabel,
}: TitleBarProps) {
  const desktop = isDesktopShell();
  return (
    <header className="titlebar" aria-label={`${pageLabel} window controls`}>
      <div className="titlebar__drag" data-tauri-drag-region="true" aria-hidden="true" />

      <div className="titlebar__right">
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
