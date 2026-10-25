import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Sidebar } from "./Sidebar";

const account = { id: "user-1", name: "Nexora User", email: "user@example.test", emailVerified: true };
function renderMenu(logout = vi.fn().mockResolvedValue(undefined)) {
  render(<Sidebar currentRoute="chat" status={null} statusFailed={false} collapsed={false} onToggleSidebar={vi.fn()} onNewChat={vi.fn()} user={account} onLogout={logout} />);
  return logout;
}
describe("sidebar account menu", () => {
  beforeEach(() => { window.location.hash = "#/chat"; });
  it("keeps Search Chats, Founder Control and account navigation out of the guest sidebar", () => {
    render(<Sidebar currentRoute="chat" status={null} statusFailed={false} collapsed={false} onToggleSidebar={vi.fn()}
      founderModeEnabled={true} onNewChat={vi.fn()} onOpenAuthModal={vi.fn()} />);
    const nav = screen.getByRole("navigation", { name: "Primary" });
    expect(nav.textContent).toContain("AI Chat");
    for (const label of ["Search chats", "Founder Control", "Plans", "Settings", "Profile"]) {
      expect(nav.textContent).not.toContain(label);
    }
    expect(screen.queryByTestId("sidebar-search-button")).toBeNull();
    expect(screen.getByRole("button", { name: "Sign in" })).toBeTruthy();
  });
  it("uses the original mark as the collapsed rail control and the name as the expanded control", async () => {
    const toggle = vi.fn();
    const actor = userEvent.setup();
    const { rerender } = render(<Sidebar currentRoute="chat" status={null} statusFailed={false}
      collapsed={true} onToggleSidebar={toggle} onNewChat={vi.fn()} />);
    const expand = screen.getByRole("button", { name: "Expand sidebar" });
    expect(expand.querySelector("svg")).toBeTruthy();
    expect(expand.textContent).toBe("");
    expect(expand.getAttribute("aria-expanded")).toBe("false");
    // Labels stay in the accessibility tree, but use the rail's visually hidden class.
    expect(screen.getByRole("link", { name: "AI Chat" }).querySelector(".nav-item__label")).toBeTruthy();
    await actor.click(expand);
    expect(toggle).toHaveBeenCalledOnce();
    rerender(<Sidebar currentRoute="chat" status={null} statusFailed={false}
      collapsed={false} onToggleSidebar={toggle} onNewChat={vi.fn()} />);
    const collapse = screen.getByRole("button", { name: "Collapse sidebar" });
    expect(collapse.textContent).toBe("Nexora");
    expect(collapse.querySelector("svg")).toBeNull();
    await actor.click(collapse);
    expect(toggle).toHaveBeenCalledTimes(2);
  });

  it("opens the signed-in identity and only the supported actions; navigates to profile", async () => {
    renderMenu();
    const actor = userEvent.setup();
    await actor.click(screen.getByRole("button", { name: "Open account menu" }));
    const menu = screen.getByRole("menu", { name: "Account" });
    expect(menu.textContent).toContain("Nexora User");
    expect(screen.getByRole("menuitem", { name: /Explore Nexora Plans/ })).toBeTruthy();
    expect(screen.queryByRole("menuitem", { name: /Help|Personalization/ })).toBeNull();
    await actor.click(screen.getByRole("menuitem", { name: "Profile" }));
    expect(window.location.hash).toBe("#/profile");
    expect(screen.queryByRole("menu")).toBeNull();
  });
  it("dismisses on Escape and logs out through the provided secure handler", async () => {
    const logout = renderMenu();
    const actor = userEvent.setup();
    await actor.click(screen.getByRole("button", { name: "Open account menu" }));
    await actor.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    await actor.click(screen.getByRole("button", { name: "Open account menu" }));
    await actor.click(screen.getByRole("menuitem", { name: "Log out" }));
    await waitFor(() => expect(logout).toHaveBeenCalledOnce());
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
  });
});
