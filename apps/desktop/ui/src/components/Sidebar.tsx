import type { AuthUser } from "../lib/auth";
import type { ConversationSummary } from "../lib/history";
import type { AppStatus } from "../lib/ipc";
import type { AnyRouteId, RouteId } from "../lib/router";
import { ApertureMark } from "./ApertureMark";
import { EngineStatus } from "./EngineStatus";
import { HistoryList } from "./HistoryList";
import { Icon, type IconName } from "./Icon";
import { RouterLink } from "./RouterLink";

/** Primary workspace destinations — every item links to a working view. */
const NAV_ITEMS: Array<{ id: RouteId; label: string; icon: IconName }> = [
  { id: "home", label: "Home", icon: "home" },
  { id: "chat", label: "AI Chat", icon: "message" },
  { id: "projects", label: "Projects", icon: "folder" },
  { id: "code", label: "Code Intelligence", icon: "code" },
  { id: "models", label: "Local Models", icon: "chip" },
  { id: "plans", label: "Plans", icon: "activity" },
  { id: "settings", label: "Settings", icon: "settings" },
];

interface SidebarProps {
  /** Active page, used for `aria-current` and the highlighted nav item. */
  currentRoute: AnyRouteId;
  status: AppStatus | null;
  statusFailed: boolean;
  collapsed: boolean;
  founderModeEnabled?: boolean;
  onNewChat: () => void;
  onOpenSearch?: () => void;
  onUnload?: () => void;
  unloading?: boolean;
  // History
  conversations?: ConversationSummary[];
  activeId?: string | null;
  historyLoading?: boolean;
  historyError?: string | null;
  onSelectConversation?: (id: string) => void;
  onRenameConversation?: (id: string, newTitle: string) => Promise<void>;
  onDeleteConversation?: (id: string) => Promise<void>;
  onRetryHistory?: () => void;
  // Optional Account / Sign-in entry
  user?: AuthUser | null;
  onOpenAuthModal?: (mode?: "signin" | "signup") => void;
}

export function Sidebar({
  currentRoute,
  status,
  statusFailed,
  collapsed,
  founderModeEnabled = false,
  onNewChat,
  onOpenSearch,
  onUnload,
  unloading = false,
  conversations = [],
  activeId = null,
  historyLoading = false,
  historyError = null,
  onSelectConversation,
  onRenameConversation,
  onDeleteConversation,
  onRetryHistory,
  user = null,
  onOpenAuthModal,
}: SidebarProps) {
  const navItems = [
    ...NAV_ITEMS,
    ...(founderModeEnabled
      ? [{ id: "founder" as RouteId, label: "Founder Control", icon: "terminal" as IconName }]
      : []),
  ];

  return (
    <aside id="sidebar" className="sidebar" aria-label="Workspace">
      <div className="sidebar__brand">
        <span className="sidebar__mark" aria-hidden="true">
          <ApertureMark size={19} />
        </span>
        <span className="sidebar__brand-text">
          <span className="sidebar__name">Nexora</span>
          <span className="sidebar__tagline">Local coding assistant</span>
        </span>
      </div>

      <div className="sidebar__actions">
        <button
          type="button"
          className="new-chat"
          aria-label="New chat"
          aria-keyshortcuts="Control+N"
          title="New chat (Ctrl+N)"
          onClick={onNewChat}
        >
          <Icon name="edit" size={15} />
          <span className="new-chat__label">New chat</span>
        </button>

        {onOpenSearch ? (
          <button
            type="button"
            className="sidebar__search-btn"
            aria-label="Search conversations"
            aria-keyshortcuts="Control+K"
            title="Search conversations (Ctrl+K)"
            data-testid="sidebar-search-button"
            onClick={onOpenSearch}
          >
            <Icon name="search" size={15} />
            <span className="sidebar__search-label">Search chats</span>
            {!collapsed ? <kbd className="sidebar__shortcut">Ctrl+K</kbd> : null}
          </button>
        ) : null}
      </div>

      <nav className="sidebar__nav" aria-label="Primary">
        <ul className="sidebar__nav-list">
          {navItems.map((item) => {
            const active = currentRoute === item.id;
            return (
              <li key={item.id}>
                <RouterLink
                  to={item.id}
                  className={`nav-item${active ? " nav-item--active" : ""}`}
                  aria-current={active ? "page" : undefined}
                  aria-label={item.label}
                  title={item.label}
                >
                  <Icon name={item.icon} size={15} />
                  <span className="nav-item__label">{item.label}</span>
                </RouterLink>
              </li>
            );
          })}
        </ul>
      </nav>

      {onSelectConversation ? (
        <HistoryList
          conversations={conversations}
          activeId={activeId}
          loading={historyLoading}
          error={historyError}
          collapsed={collapsed}
          onSelect={onSelectConversation}
          onRename={onRenameConversation ?? (async () => {})}
          onDelete={onDeleteConversation ?? (async () => {})}
          onRetry={onRetryHistory ?? (() => {})}
        />
      ) : (
        <div className="sidebar__section">
          <h2 className="sidebar__label">Recent</h2>
          <p className="sidebar__empty">
            No conversations yet. History is kept for this session only.
          </p>
        </div>
      )}

      <div className="sidebar__footer">
        <EngineStatus
          status={status}
          failed={statusFailed}
          collapsed={collapsed}
          onUnload={onUnload}
          unloading={unloading}
        />

        {user ? (
          <RouterLink
            to="account"
            className={`sidebar__account${currentRoute === "account" ? " sidebar__account--active" : ""}`}
            aria-label="Developer Account"
            title={user.name ? `${user.name} (${user.email})` : user.email}
          >
            <Icon name="user" size={15} />
            {!collapsed ? (
              <span className="sidebar__account-name">Account</span>
            ) : null}
          </RouterLink>
        ) : onOpenAuthModal ? (
          <button
            type="button"
            className="sidebar__signin-btn"
            aria-label="Sign in"
            title="Sign in (optional — local chat works without signing in)"
            onClick={() => onOpenAuthModal("signin")}
          >
            <Icon name="user" size={15} />
            {!collapsed ? <span className="sidebar__account-name">Sign in</span> : null}
          </button>
        ) : null}

        <div className="sidebar__legal">
          <span>Nexora {status?.version ?? "0.1.0"}</span>
          <span aria-hidden="true">·</span>
          <span>Runs on device</span>
        </div>
      </div>
    </aside>
  );
}
