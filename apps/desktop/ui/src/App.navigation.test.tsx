import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./lib/inference", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/inference")>();

  return {
    ...actual,
    fetchStatus: vi.fn(),
    fetchSettings: vi.fn().mockResolvedValue({
      modelPath: null,
      runtimePath: null,
      contextSize: 32768,
    }),
    fetchSystemInfo: vi.fn().mockResolvedValue({
      totalRamGib: 32,
      availableRamGib: 20.5,
      platform: "windows",
    }),
    generate: vi.fn(),
    cancelGeneration: vi.fn().mockResolvedValue(true),
    unloadModel: vi.fn(),
  };
});

vi.mock("./lib/history", () => ({
  listConversations: async () => [],
  createConversation: async () => ({ id: "new-chat-id", title: "New conversation", createdAt: Date.now(), updatedAt: Date.now(), messages: [] }),
  saveConversation: async (c: import("./lib/history").PersistedConversation) => c,
  loadConversation: async () => { throw { code: "not_found", message: "Not found" }; },
  renameConversation: async () => { throw { code: "not_found", message: "Not found" }; },
  deleteConversation: async () => true,
}));

import App from "./App";
import { fetchStatus, fetchSystemInfo } from "./lib/inference";
import { makeStatus, markDesktop } from "./test/fixtures";

const statusMock = vi.mocked(fetchStatus);
const systemMock = vi.mocked(fetchSystemInfo);

/** Sets the route the way a deep link would, before render. */
function goto(hash: string) {
  act(() => {
    window.location.hash = hash;
    window.dispatchEvent(new Event("hashchange"));
  });
}

beforeEach(() => {
  statusMock.mockReset();
  statusMock.mockResolvedValue(makeStatus({ state: "ready" }));
  systemMock.mockReset();
  systemMock.mockResolvedValue({
    totalRamGib: 32,
    availableRamGib: 20.5,
    platform: "windows",
  });
  markDesktop();
  // Phase 20: the app opens on Welcome; these suites cover the workspace pages.
  window.location.hash = "#/home";
});

describe("Phase 19 page navigation", () => {
  it("opens on the Home page with real quick actions", async () => {
    render(<App />);

    expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Start a new chat/i })).toBeTruthy();
    const nav = screen.getByRole("navigation", { name: "Primary" });
    expect(within(nav).getByRole("link", { name: "Code Intelligence" })).toBeTruthy();
    // Home must not embed the chat transcript or the projects interface.
    expect(screen.queryByRole("region", { name: "Conversation messages" })).toBeNull();
    expect(screen.queryByRole("complementary", { name: "Project explorer" })).toBeNull();
  });

  it("keeps the shared shell on every page", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Welcome to Nexora" });

    const nav = screen.getByRole("navigation", { name: "Primary" });
    for (const name of ["Home", "AI Chat", "Projects", "Code Intelligence", "Local Models", "App Integrations"]) {
      expect(within(nav).getByRole("link", { name })).toBeTruthy();
    }
    expect(screen.getByRole("button", { name: "New chat" })).toBeTruthy();
  });

  it("every navigation item leads to its correct page", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: "Welcome to Nexora" });

    const nav = screen.getByRole("navigation", { name: "Primary" });

    await user.click(within(nav).getByRole("link", { name: "AI Chat" }));
    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();

    await user.click(within(nav).getByRole("link", { name: "Projects" }));
    expect(await screen.findByRole("complementary", { name: "Project explorer" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Conversation messages" })).toBeNull();

    await user.click(within(nav).getByRole("link", { name: "Code Intelligence" }));
    expect(await screen.findByRole("heading", { name: "Code intelligence" })).toBeTruthy();

    await user.click(within(nav).getByRole("link", { name: "Local Models" }));
    expect(await screen.findByRole("heading", { name: "Local models" })).toBeTruthy();

    await user.click(within(nav).getByRole("link", { name: "App Integrations" }));
    expect(await screen.findByRole("heading", { name: "App Integrations" })).toBeTruthy();
    expect(window.location.hash).toBe("#/integrations");
    expect(screen.queryByRole("form", { name: "Message composer" })).toBeNull();
    await user.click(within(nav).getByRole("link", { name: "AI Chat" }));
    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();
    expect(window.location.hash).toBe("#/chat");

    // Settings is account-only; direct guest URLs must not render it.
    goto("#/settings");
    expect(await screen.findByRole("region", { name: "Sign in required" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Settings" })).toBeNull();

    await user.click(within(nav).getByRole("link", { name: "Home" }));
    expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
  });

  it("marks the active page with aria-current and a visible nav state", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: "Welcome to Nexora" });

    const nav = screen.getByRole("navigation", { name: "Primary" });
    const homeLink = within(nav).getByRole("link", { name: "Home" });
    expect(homeLink.getAttribute("aria-current")).toBe("page");

    await user.click(within(nav).getByRole("link", { name: "Projects" }));
    const projectsLink = within(nav).getByRole("link", { name: "Projects" });
    expect(projectsLink.getAttribute("aria-current")).toBe("page");
    expect(projectsLink.className).toContain("nav-item--active");
    expect(homeLink.getAttribute("aria-current")).toBeNull();
  });

  it("keeps AI Chat and Projects as separate views", async () => {
    render(<App />);

    // On chat: no projects interface.
    goto("#/chat");
    await screen.findByRole("region", { name: "Conversation messages" });
    expect(screen.queryByRole("complementary", { name: "Project explorer" })).toBeNull();

    // On projects: no chat transcript or composer.
    goto("#/projects");
    await screen.findByRole("complementary", { name: "Project explorer" });
    expect(screen.queryByRole("region", { name: "Conversation messages" })).toBeNull();
    expect(screen.queryByRole("form", { name: "Message composer" })).toBeNull();
  });

  it("navigates by hash so pages switch without reloading the app", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: "Welcome to Nexora" });

    const nav = screen.getByRole("navigation", { name: "Primary" });
    const chatLink = within(nav).getByRole("link", { name: "AI Chat" });
    expect(chatLink.getAttribute("href")).toBe("#/chat");

    await user.click(chatLink);
    expect(window.location.hash).toBe("#/chat");
  });

  it("preserves the conversation draft across page switches", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/chat");

    const composer = await screen.findByRole("textbox", { name: "Message Nexora" });
    await user.type(composer, "remember this draft");

    goto("#/projects");
    await screen.findByRole("complementary", { name: "Project explorer" });

    goto("#/chat");
    expect(
      (await screen.findByRole("textbox", { name: "Message Nexora" }) as HTMLTextAreaElement).value,
    ).toContain("remember this draft");
  });

  it("shows a graceful fallback for unknown routes with a way back", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/no-such-page");

    expect(await screen.findByRole("heading", { name: "Page not found." })).toBeTruthy();
    expect(screen.getByText("/no-such-page")).toBeTruthy();

    await user.click(screen.getByRole("link", { name: /Return to Home/i }));
    expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
    expect(window.location.hash).toBe("#/home");
  });

  it("starts a new chat from the Home page and lands on AI Chat", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("heading", { name: "Welcome to Nexora" });

    await user.click(screen.getByRole("button", { name: /Start a new chat/i }));

    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();
    expect(window.location.hash).toBe("#/chat");
  });

  it("keeps Ctrl+N / Ctrl+L / Ctrl+B working from any page", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Welcome to Nexora" });

    // Ctrl+B toggles the sidebar from Home.
    const toggle = screen.getByRole("button", { name: "Collapse sidebar" });
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "b", ctrlKey: true }));
    });
    expect(
      screen.getByRole("button", { name: "Expand sidebar" }).getAttribute("aria-expanded"),
    ).toBe("false");

    // Ctrl+N starts a new chat and routes to the AI Chat page.
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "n", ctrlKey: true }));
    });
    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();
    expect(window.location.hash).toBe("#/chat");

    // Ctrl+L focuses the composer, routing to chat first when needed.
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "l", ctrlKey: true }));
    });
    await screen.findByRole("textbox", { name: "Message Nexora" });
    expect(document.activeElement).toBe(
      screen.getByRole("textbox", { name: "Message Nexora" }),
    );
  });

  it("links the Home engine card to Local Models when no model is ready", async () => {
    statusMock.mockResolvedValue(makeStatus({ state: "not_configured" }));
    const user = userEvent.setup();
    render(<App />);

    await screen.findByRole("heading", { name: "Welcome to Nexora" });
    await user.click(screen.getAllByRole("link", { name: "Open local models" })[0]!);

    expect(await screen.findByRole("heading", { name: "Local models" })).toBeTruthy();
    expect(window.location.hash).toBe("#/models");
  });
});
