import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { navigateTo } from "../lib/router";
import { AccountAvatar } from "./AccountAvatar";
import { accountGetProfile, isAccountProfile, type AuthUser } from "../lib/auth";
import type { ConversationSummary } from "../lib/history";
import type { AppStatus } from "../lib/ipc";
import type { AnyRouteId, RouteId } from "../lib/router";
import { NexoraMark } from "./NexoraMark";
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
  { id: "integrations", label: "App Integrations", icon: "apps" },
  { id: "plans", label: "Plans", icon: "activity" },
  { id: "settings", label: "Settings", icon: "settings" },
];

interface SidebarProps {
  /** Active page, used for `aria-current` and the highlighted nav item. */
  currentRoute: AnyRouteId;
  status: AppStatus | null;
  statusFailed: boolean;
  collapsed: boolean;
  onToggleSidebar: () => void;
  founderModeEnabled?: boolean;
  onNewChat: () => void;
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
  onLogout?: () => Promise<void>;
}

export function Sidebar({
  currentRoute,
  status,
  statusFailed,
  collapsed,
  onToggleSidebar,
  founderModeEnabled = false,
  onNewChat,
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
  onLogout,
}: SidebarProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [planRecord, setPlanRecord] = useState<{ accountId: string; name: string } | null>(null);
  useEffect(() => {
    let active = true;
    setPlanRecord(null);
    if (user?.id) void accountGetProfile().then((result) => {
      if (active && isAccountProfile(result) && result.id === user.id && result.planName.trim()) {
        setPlanRecord({ accountId: user.id, name: result.planName.trim() });
      }
    }).catch(() => { /* Plan status unavailable; never assume Free or paid access. */ });
    return () => { active = false; };
  }, [user?.id]);
  const [logoutError, setLogoutError] = useState<string | null>(null);
  const [loggingOut, setLoggingOut] = useState(false);
  const avatarRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [menuPosition, setMenuPosition] = useState({ left: 8, bottom: 8 });

  useEffect(() => { setMenuOpen(false); setLogoutError(null); }, [currentRoute, user?.id]);
  useEffect(() => {
    if (!menuOpen) return;
    const closeOutside = (event: PointerEvent) => {
      if (menuRef.current?.contains(event.target as Node) || avatarRef.current?.contains(event.target as Node)) return;
      setMenuOpen(false);
    };
    const closeEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") { setMenuOpen(false); avatarRef.current?.focus(); }
    };
    const closeOnScroll = () => setMenuOpen(false);
    document.addEventListener("pointerdown", closeOutside);
    document.addEventListener("keydown", closeEscape);
    window.addEventListener("resize", closeOnScroll);
    menuRef.current?.querySelector<HTMLButtonElement>("button")?.focus();
    return () => {
      document.removeEventListener("pointerdown", closeOutside);
      document.removeEventListener("keydown", closeEscape);
      window.removeEventListener("resize", closeOnScroll);
    };
  }, [menuOpen]);

  const toggleMenu = () => {
    if (!menuOpen && avatarRef.current) {
      const rect = avatarRef.current.getBoundingClientRect();
      setMenuPosition({ left: Math.max(8, Math.min(rect.left, window.innerWidth - Math.min(320, window.innerWidth - 16) - 8)), bottom: Math.max(8, window.innerHeight - rect.top + 8) });
    }
    setMenuOpen((open) => !open);
  };
  const openPage = (page: "plans" | "profile" | "settings") => {
    setMenuOpen(false);
    navigateTo(page);
  };
  const signOut = async () => {
    if (!onLogout) return;
    setLoggingOut(true);
    setLogoutError(null);
    try { await onLogout(); setMenuOpen(false); navigateTo("chat"); }
    catch { setLogoutError("Could not log out. Please try again."); }
    finally { setLoggingOut(false); }
  };

  const navItems = [
    ...NAV_ITEMS.filter((item) => user || (item.id !== "plans" && item.id !== "settings")),
    ...(founderModeEnabled && user
      ? [{ id: "founder" as RouteId, label: "Founder Control", icon: "terminal" as IconName }]
      : []),
  ];

  return (
    <aside id="sidebar" className="sidebar" aria-label="Workspace">
      <div className="sidebar__brand">
        <button
          type="button"
          className="sidebar__brand-toggle"
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-expanded={!collapsed}
          aria-controls="sidebar"
          aria-keyshortcuts="Control+B"
          title={`${collapsed ? "Expand" : "Collapse"} sidebar (Ctrl+B)`}
          onClick={onToggleSidebar}
        >
          {collapsed ? <NexoraMark size={24} /> : <span className="sidebar__name">Nexora</span>}
        </button>
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
          <>
            <button ref={avatarRef} type="button" className="sidebar__account" aria-label="Open account menu" aria-haspopup="menu" aria-expanded={menuOpen} onClick={toggleMenu} title={user.name || user.email}>
              <AccountAvatar user={user} />
              {!collapsed ? <span className="sidebar__account-name">{user.name || user.email}</span> : null}
              {!collapsed ? <Icon name="chevron-down" size={14} /> : null}
            </button>
            {menuOpen && createPortal(
              <div ref={menuRef} className="account-menu" role="menu" aria-label="Account" style={{ left: menuPosition.left, bottom: menuPosition.bottom }}>
                <div className="account-menu__identity"><AccountAvatar user={user} /><span className="account-menu__identity-text"><span className="account-menu__name">{user.name || user.email}</span>{planRecord?.accountId === user.id && <span className="account-menu__plan">{planRecord.name} · account record</span>}</span></div>
                <div className="account-menu__divider" />
                <button role="menuitem" onClick={() => openPage("plans")}><Icon name="activity" size={18} /> Explore Nexora Plans</button>
                <button role="menuitem" onClick={() => openPage("profile")}><Icon name="user" size={18} /> Profile</button>
                <button role="menuitem" onClick={() => openPage("settings")}><Icon name="settings" size={18} /> Settings</button>
                <div className="account-menu__divider" />
                <button role="menuitem" disabled={loggingOut} onClick={() => void signOut()}><Icon name="log-out" size={18} /> {loggingOut ? "Logging out…" : "Log out"}</button>
                {logoutError && <p role="alert" className="account-menu__error">{logoutError}</p>}
              </div>, document.body
            )}
          </>
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
