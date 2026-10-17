import type { AppStatus } from "../lib/ipc";
import { ApertureMark } from "./ApertureMark";
import { EngineStatus } from "./EngineStatus";
import { Icon } from "./Icon";

interface SidebarProps {
  status: AppStatus | null;
  statusFailed: boolean;
  collapsed: boolean;
  onNewChat: () => void;
}

export function Sidebar({
  status,
  statusFailed,
  collapsed,
  onNewChat,
}: SidebarProps) {
  return (
    <aside id="sidebar" className="sidebar" aria-label="Workspace">
      <div className="sidebar__brand">
        <span className="sidebar__mark" aria-hidden="true">
          <ApertureMark size={22} />
        </span>
        <span className="sidebar__brand-text">
          <span className="sidebar__name">Nexora</span>
          <span className="sidebar__tagline">Local coding studio</span>
        </span>
      </div>

      <button
        type="button"
        className="new-chat"
        aria-label="New chat"
        aria-keyshortcuts="Control+N"
        title="New chat (Ctrl+N)"
        onClick={onNewChat}
      >
        <Icon name="plus" size={16} />
        <span className="new-chat__label">New chat</span>
      </button>

      <div className="sidebar__section">
        <h2 className="sidebar__label">Recent</h2>
        <p className="sidebar__empty">
          No conversations yet. History is kept for this session only.
        </p>
      </div>

      <div className="sidebar__footer">
        <EngineStatus
          status={status}
          failed={statusFailed}
          collapsed={collapsed}
        />
        <div className="sidebar__legal">
          <span>Nexora {status?.version ?? "0.1.0"}</span>
          <span aria-hidden="true">·</span>
          <span>Scaffold build</span>
        </div>
      </div>
    </aside>
  );
}
