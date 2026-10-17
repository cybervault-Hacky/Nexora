import type { AppStatus } from "../lib/ipc";
import { ApertureMark } from "./ApertureMark";
import { EngineStatus } from "./EngineStatus";
import { Icon } from "./Icon";

interface SidebarProps {
  status: AppStatus | null;
  onNewChat: () => void;
}

export function Sidebar({ status, onNewChat }: SidebarProps) {
  return (
    <aside className="sidebar" aria-label="Workspace">
      <div className="sidebar__brand">
        <span className="sidebar__mark">
          <ApertureMark size={22} />
        </span>
        <span className="sidebar__brand-text">
          <span className="sidebar__name">Nexora</span>
          <span className="sidebar__tagline">Local coding studio</span>
        </span>
      </div>

      <button type="button" className="new-chat" onClick={onNewChat}>
        <Icon name="plus" size={16} />
        <span>New chat</span>
      </button>

      <div className="sidebar__section">
        <h2 className="sidebar__label">Recent</h2>
        <p className="sidebar__empty">
          No conversations yet. Your work stays on this machine.
        </p>
      </div>

      <div className="sidebar__footer">
        <EngineStatus status={status} />
        <div className="sidebar__legal">
          <span>Nexora {status?.version ?? "0.1.0"}</span>
          <span aria-hidden="true">·</span>
          <span>Scaffold build</span>
        </div>
      </div>
    </aside>
  );
}
