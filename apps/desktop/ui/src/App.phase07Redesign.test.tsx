import fs from "node:fs";
import path from "node:path";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("./lib/inference", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/inference")>();
  return {
    ...actual,
    fetchStatus: vi.fn(),
    fetchSettings: vi.fn().mockResolvedValue({
      modelPath: "C:\\models\\qwen2.5-coder-7b-instruct-q4_k_m.gguf",
      runtimePath: "C:\\llama\\llama-server.exe",
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

import App from "./App";
import { AssistantContent } from "./components/MessageContent";
import { searchLocalConversations } from "./features/chat/searchConversations";
import type { ConversationSummary, PersistedConversation } from "./lib/history";
import { fetchStatus, generate } from "./lib/inference";
import { PlansPage } from "./pages/PlansPage";
import { makeStatus, makeUsage, markDesktop } from "./test/fixtures";

const statusMock = vi.mocked(fetchStatus);
const generateMock = vi.mocked(generate);

function contrastRatio(hexA: string, hexB: string): number {
  const lum = (hex: string) => {
    const r = parseInt(hex.slice(1, 3), 16) / 255;
    const g = parseInt(hex.slice(3, 5), 16) / 255;
    const b = parseInt(hex.slice(5, 7), 16) / 255;
    const f = (c: number) => (c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
  };
  const l1 = lum(hexA);
  const l2 = lum(hexB);
  return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
}

describe("Phase 07 — Complete ChatGPT-Inspired UI Redesign Acceptance Suite", () => {
  let storedConversations: PersistedConversation[] = [];
  let listError: string | null = null;
  let originalFetch: typeof globalThis.fetch;
  let fetchSpy: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    listError = null;
    storedConversations = [
      {
        id: "conv-borrow-checker",
        title: "Rust Borrow Checker Lifetimes",
        createdAt: 1_760_000_000_000,
        updatedAt: 1_760_000_200_000,
        messages: [
          {
            id: "m1",
            role: "user",
            content: "Why does mutable aliasing fail inside my iterator?",
            createdAt: 1_760_000_010_000,
            state: "complete",
          },
          {
            id: "m2",
            role: "assistant",
            content: "Split the struct borrow or iterate with `.iter_mut()`.",
            createdAt: 1_760_000_020_000,
            state: "complete",
          },
        ],
      },
      {
        id: "conv-sse-framer",
        title: "Streaming Parser Design",
        createdAt: 1_760_000_300_000,
        updatedAt: 1_760_000_500_000,
        messages: [
          {
            id: "m3",
            role: "user",
            content: "Buffer incomplete UTF-8 byte frames across TCP reads",
            createdAt: 1_760_000_310_000,
            state: "complete",
          },
          {
            id: "m4",
            role: "assistant",
            content: "Hold raw bytes in a ring buffer until `\\n` arrives before decoding UTF-8.",
            createdAt: 1_760_000_320_000,
            state: "complete",
          },
        ],
      },
    ];

    originalFetch = globalThis.fetch;
    fetchSpy = vi.fn().mockRejectedValue(new Error("Unexpected network fetch"));
    globalThis.fetch = fetchSpy as unknown as typeof globalThis.fetch;
    window.fetch = globalThis.fetch;

    window.localStorage.clear();
    document.documentElement.dataset.theme = "dark";
    markDesktop(true);
    window.location.hash = "";
    act(() => {
      window.dispatchEvent(new Event("hashchange"));
    });

    statusMock.mockReset();
    statusMock.mockResolvedValue(makeStatus({ state: "ready" }));
    generateMock.mockReset();

    invokeMock.mockReset();
    invokeMock.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      switch (cmd) {
        case "auth_get_session":
          return { user: null, authenticated: false };
        case "get_developer_diagnostics":
          return {
            platform: "windows",
            arch: "x86_64",
            appVersion: "0.1.0",
            credentialVault: "Windows Credential Manager",
            authServiceUrl: "",
            cspEnforced: true,
            offlineCapable: true,
          };
        case "get_usage_status":
          return makeUsage();
        case "list_conversations":
          if (listError) {
            throw { code: "storage", message: listError };
          }
          return storedConversations.map(
            (c): ConversationSummary => ({
              id: c.id,
              title: c.title,
              createdAt: c.createdAt,
              updatedAt: c.updatedAt,
              messageCount: c.messages.length,
              preview: c.messages[c.messages.length - 1]?.content.slice(0, 80) ?? "",
            }),
          );
        case "load_conversation": {
          const id = String(args?.id ?? "");
          const found = storedConversations.find((c) => c.id === id);
          if (!found) throw { code: "not_found", message: "Conversation not found" };
          return found;
        }
        case "create_conversation": {
          const created: PersistedConversation = {
            id: (args?.id as string | undefined) ?? `chat-${Date.now()}`,
            title: (args?.title as string | undefined) ?? "New conversation",
            createdAt: Date.now(),
            updatedAt: Date.now(),
            messages: [],
          };
          storedConversations.unshift(created);
          return created;
        }
        case "save_conversation": {
          const conv = args?.conversation as PersistedConversation;
          const idx = storedConversations.findIndex((item) => item.id === conv.id);
          if (idx >= 0) storedConversations[idx] = conv;
          else storedConversations.unshift(conv);
          return conv;
        }
        case "billing_list_plans":
          throw {
            code: "backend_unconfigured",
            message: "Account service URL is not configured in this local build.",
          };
        default:
          return null;
      }
    });
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    window.fetch = originalFetch;
    markDesktop(false);
  });

  it("defines exact ChatGPT neutral Dark & Light tokens with WCAG AA/AAA contrast and zero-flash boot script", () => {
    const tokensCss = fs.readFileSync(path.resolve(__dirname, "styles/tokens.css"), "utf-8");
    const indexHtml = fs.readFileSync(path.resolve(__dirname, "../index.html"), "utf-8");
    const themeInitJs = fs.readFileSync(path.resolve(__dirname, "../public/theme-init.js"), "utf-8");

    // Dark reference palette
    expect(tokensCss).toContain("--bg-base: #212121;");
    expect(tokensCss).toContain("--bg-sidebar: #171717;");
    expect(tokensCss).toContain("--bg-panel: #2f2f2f;");
    expect(tokensCss).toContain("--border-default: #424242;");
    expect(tokensCss).toContain("--text-primary: #f3f3f3;");
    expect(tokensCss).toContain("--text-secondary: #afafaf;");

    // Light reference palette
    expect(tokensCss).toContain("--bg-base: #ffffff;");
    expect(tokensCss).toContain("--bg-sidebar: #f9f9f9;");
    expect(tokensCss).toContain("--bg-panel: #f4f4f4;");
    expect(tokensCss).toContain("--border-default: #e5e5e5;");
    expect(tokensCss).toContain("--text-primary: #202123;");
    expect(tokensCss).toContain("--text-secondary: #6b6b6b;");

    // WCAG contrast checks across both themes
    expect(contrastRatio("#212121", "#f3f3f3")).toBeGreaterThan(13);
    expect(contrastRatio("#212121", "#afafaf")).toBeGreaterThan(7);
    expect(contrastRatio("#ffffff", "#202123")).toBeGreaterThan(15);
    expect(contrastRatio("#ffffff", "#6b6b6b")).toBeGreaterThan(5);

    // Pre-hydration external script preserves strict `script-src 'self'` CSP with zero flash
    expect(indexHtml).toContain('<script src="./theme-init.js"></script>');
    expect(themeInitJs).toContain("document.documentElement.dataset.theme");
  });

  it("persists Light/Dark theme selection across app remounts without flashing", async () => {
    const user = userEvent.setup();
    const first = render(<App />);

    await user.click(await screen.findByRole("button", { name: "Switch to light theme" }));
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(window.localStorage.getItem("nexora.theme")).toBe("light");

    first.unmount();

    render(<App />);
    expect(await screen.findByRole("button", { name: "Switch to dark theme" })).toBeTruthy();
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("searches local titles and message contents via Sidebar trigger and Ctrl+K with keyboard navigation and draft preservation", async () => {
    const user = userEvent.setup();
    render(<App />);

    const composer = (await screen.findByRole("textbox", {
      name: "Message Nexora",
    })) as HTMLTextAreaElement;
    await user.type(composer, "Keep this unsent draft safe");

    // Open search via Sidebar trigger
    await user.click(screen.getByTestId("sidebar-search-button"));
    const modal = await screen.findByTestId("conversation-search-modal");
    const searchInput = within(modal).getByRole("combobox", { name: "Search conversations" });

    // Query text inside a historical message that does NOT appear in title or last-message preview
    await user.type(searchInput, "mutable aliasing");

    expect(await within(modal).findByText("Rust Borrow Checker Lifetimes")).toBeTruthy();
    expect(within(modal).queryByText("Streaming Parser Design")).toBeNull();

    // Select with Enter key -> loads conversation into Chat while keeping unsent draft intact
    await user.keyboard("{Enter}");
    await waitFor(() => {
      expect(screen.queryByTestId("conversation-search-modal")).toBeNull();
    });
    expect(
      await within(screen.getByRole("main")).findByText(/Split the struct borrow/),
    ).toBeTruthy();
    expect(composer.value).toBe("Keep this unsent draft safe");

    // Re-open with Ctrl+K and verify no-results empty state + Escape close
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true }));
    });
    const modalViaShortcut = await screen.findByTestId("conversation-search-modal");
    const input2 = within(modalViaShortcut).getByRole("combobox", {
      name: "Search conversations",
    });
    await user.type(input2, "nonexistent-search-needle-xyz");
    expect(await within(modalViaShortcut).findByTestId("search-empty-state")).toBeTruthy();

    await user.keyboard("{Escape}");
    expect(screen.queryByTestId("conversation-search-modal")).toBeNull();
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("drops stale async conversation search lookups when superseded by a newer query", async () => {
    const summaries: ConversationSummary[] = [
      {
        id: "slow-a",
        title: "Topic Alpha",
        createdAt: 1,
        updatedAt: 2,
        messageCount: 1,
        preview: "",
      },
      {
        id: "fast-b",
        title: "Topic Beta",
        createdAt: 3,
        updatedAt: 4,
        messageCount: 1,
        preview: "",
      },
    ];

    let cancelledFirstQuery = false;
    let resolveSlow!: (conv: PersistedConversation) => void;
    const slowPromise = new Promise<PersistedConversation>((resolve) => {
      resolveSlow = resolve;
    });

    const firstSearch = searchLocalConversations(
      summaries,
      "alpha",
      () => cancelledFirstQuery,
      async () => slowPromise,
    );

    // Supersede immediately before `slowPromise` resolves
    cancelledFirstQuery = true;
    const newerHits = await searchLocalConversations(
      summaries,
      "beta",
      () => false,
      async (id) => ({
        id,
        title: id === "fast-b" ? "Topic Beta" : "Topic Alpha",
        createdAt: 1,
        updatedAt: 2,
        messages: [],
      }),
    );

    resolveSlow({
      id: "slow-a",
      title: "Topic Alpha",
      createdAt: 1,
      updatedAt: 2,
      messages: [],
    });
    const staleResult = await firstSearch;

    expect(staleResult).toEqual([]);
    expect(newerHits.map((h) => h.conversation.id)).toEqual(["fast-b"]);
  });

  it("renders headings, lists, scrollable tables, code blocks, and sanitized links safely in AssistantContent", () => {
    const markdown = [
      "## Architecture Summary",
      "",
      "- Zero cloud telemetry",
      "- Verified `SHA-256` weights",
      "",
      "| Profile | Context |",
      "| --- | --- |",
      "| Nexora Mini | 32768 |",
      "",
      "See [Rust Async Book](https://rust-lang.github.io/async-book/) and ignore [bad](javascript:alert(1)) or <script>evil()</script>.",
      "",
      "```rust",
      "fn ready() -> bool { true }",
      "```",
    ].join("\n");

    const { container } = render(<AssistantContent content={markdown} />);

    expect(container.querySelector("h2.markdown-heading--2")?.textContent).toBe(
      "Architecture Summary",
    );
    expect(container.querySelectorAll("ul.markdown-list li")).toHaveLength(2);
    expect(container.querySelector("table.markdown-table th")?.textContent).toBe("Profile");
    expect(container.querySelector("table.markdown-table td")?.textContent).toBe("Nexora Mini");

    const links = container.querySelectorAll("a.markdown-link");
    expect(links).toHaveLength(1);
    expect(links[0]?.getAttribute("href")).toBe("https://rust-lang.github.io/async-book/");
    expect(links[0]?.getAttribute("rel")).toBe("noopener noreferrer");
    expect(container.querySelector("script")).toBeNull();
    expect(container.textContent).toContain("<script>evil()</script>");
    expect(container.querySelector(".codeblock code")?.textContent).toContain(
      "fn ready() -> bool { true }",
    );
  });

  it("displays honest unconfigured Plans state with zero invented prices and explains protected auth reason when signing in", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Open Plans from Primary sidebar navigation
    const nav = await screen.findByRole("navigation", { name: "Primary" });
    await user.click(within(nav).getByRole("link", { name: "Plans" }));

    const unconfiguredCard = await screen.findByTestId("plans-unconfigured-state");
    expect(unconfiguredCard.textContent).toContain("never invents subscription prices");

    // Clicking Sign in inside Plans opens floating AuthModal with protected-action explanation
    await user.click(within(unconfiguredCard).getByRole("button", { name: "Sign in" }));
    const modal = await screen.findByTestId("auth-modal");
    expect(within(modal).getByTestId("auth-modal-reason").textContent).toContain(
      "Local AI chat remains available without signing in",
    );
    expect(within(modal).getByTestId("auth-backend-note")).toBeTruthy();
  });

  it("never fabricates $0 when a server plan omits price entries", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "billing_list_plans") {
        return {
          paymentConfigured: false,
          provider: "unconfigured",
          plans: [
            {
              id: "pro",
              name: "Pro",
              description: "Server-defined tier awaiting final pricing.",
              features: ["Expanded daily local allowance"],
              modelQuotas: {},
              prices: [],
            },
          ],
        };
      }
      return undefined;
    });

    render(<PlansPage isAuthenticated={false} onRequestSignIn={vi.fn()} />);
    const card = await screen.findByTestId("plan-card-pro");
    expect(within(card).getByText("Pricing unavailable")).toBeTruthy();
    expect(card.textContent).not.toContain("$0");
  });
});
