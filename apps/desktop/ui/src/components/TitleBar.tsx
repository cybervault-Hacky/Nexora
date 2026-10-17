import { ApertureMark } from "./ApertureMark";
import { Icon } from "./Icon";
import { isDesktopShell } from "../lib/ipc";
import {
  closeWindow,
  minimizeWindow,
  toggleMaximizeWindow,
} from "../lib/window";

export function TitleBar() {
  const desktop = isDesktopShell();

  return (
    <header
      className="titlebar"
      data-tauri-drag-region
      onDoubleClick={() => {
        void toggleMaximizeWindow();
      }}
    >
      <div className="titlebar__left" data-tauri-drag-region>
        <ApertureMark size={17} className="titlebar__mark" />
        <span className="titlebar__wordmark" aria-label="Nexora">
          nexora
        </span>
        <span className="titlebar__rule" aria-hidden="true" />
        <span className="titlebar__context">New conversation</span>
      </div>

      <div className="titlebar__right">
        <span className="badge badge--local" title="No cloud service is used">
          <span className="badge__dot" aria-hidden="true" />
          Local
        </span>

        {desktop ? (
          <div className="window-controls">
            <button
              type="button"
              className="window-control"
              aria-label="Minimize window"
              onClick={() => void minimizeWindow()}
            >
              <Icon name="minimize" size={14} />
            </button>
            <button
              type="button"
              className="window-control"
              aria-label="Maximize or restore window"
              onClick={() => void toggleMaximizeWindow()}
            >
              <Icon name="maximize" size={12} />
            </button>
            <button
              type="button"
              className="window-control window-control--close"
              aria-label="Close window"
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
