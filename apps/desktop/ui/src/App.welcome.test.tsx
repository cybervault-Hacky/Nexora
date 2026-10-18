import { act, render, screen } from "@testing-library/react";
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

/** Sets the route the way a deep link or refresh would, before render. */
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
  markDesktop(false);
  window.location.hash = "";
});

describe("Phase 20 welcome & onboarding flow", () => {
  it("opens the app on the Welcome screen", async () => {
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "Code with an AI that runs on your machine." }),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Continue" })).toBeTruthy();
    expect(screen.getByText(/llama\.cpp runtime and GGUF model/i)).toBeTruthy();
    // The flow is outside the workspace shell: no sidebar navigation or composer.
    expect(screen.queryByRole("navigation", { name: "Primary" })).toBeNull();
    expect(screen.queryByRole("form", { name: "Message composer" })).toBeNull();
  });

  it("describes local-first capabilities without a cloud claim", async () => {
    render(<App />);

    await screen.findByRole("heading", { name: "Code with an AI that runs on your machine." });
    expect(screen.getByText(/does not send them to a cloud service/i)).toBeTruthy();
    expect(screen.getByRole("list", { name: "What Nexora includes" })).toBeTruthy();
  });

  it("hides the sidebar toggle on the flow but keeps window chrome", async () => {
    render(<App />);

    await screen.findByRole("heading", { name: "Code with an AI that runs on your machine." });
    expect(screen.queryByRole("button", { name: "Collapse sidebar" })).toBeNull();
    expect(screen.getAllByText(/nexora/i).length).toBeGreaterThan(0);
  });

  it("Continue advances to the honest onboarding-next screen", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findByRole("heading", { name: "Code with an AI that runs on your machine." });
    await user.click(screen.getByRole("button", { name: "Continue" }));

    expect(window.location.hash).toBe("#/onboarding");
    expect(await screen.findByRole("heading", { name: "Sign-in is the next step." })).toBeTruthy();
    expect(screen.getByText(/not part of this build/i)).toBeTruthy();
    // Honest placeholder, not a fake authentication form.
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.queryByLabelText(/password/i)).toBeNull();
    expect(screen.getByRole("note", { name: "Upcoming sign-in" })).toBeTruthy();
  });

  it("Back returns to Welcome and the workspace action opens Home", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/onboarding");

    await screen.findByRole("heading", { name: "Sign-in is the next step." });

    await user.click(screen.getByRole("button", { name: "Back to Welcome" }));
    expect(await screen.findByRole("button", { name: "Continue" })).toBeTruthy();
    expect(window.location.hash).toBe("#/welcome");

    await user.click(screen.getByRole("button", { name: "Continue" }));
    await screen.findByRole("heading", { name: "Sign-in is the next step." });
    await user.click(screen.getByRole("button", { name: "Open the workspace" }));

    expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
    expect(window.location.hash).toBe("#/home");
    expect(screen.getByRole("navigation", { name: "Primary" })).toBeTruthy();
  });

  it("renders onboarding on direct load or refresh without breaking", async () => {
    render(<App />);
    goto("#/onboarding");

    expect(await screen.findByRole("heading", { name: "Sign-in is the next step." })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Back to Welcome" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open the workspace" })).toBeTruthy();
  });

  it("keeps unknown routes graceful from the flow entry", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/definitely-not-a-page");

    expect(await screen.findByRole("heading", { name: "Page not found." })).toBeTruthy();
    await user.click(screen.getByRole("link", { name: /Return to Home/i }));
    expect(await screen.findByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
    expect(window.location.hash).toBe("#/home");
  });

  it("supports keyboard use of the primary Continue action", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findByRole("heading", { name: "Code with an AI that runs on your machine." });

    await user.tab();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Continue" }));
    await user.keyboard("{Enter}");

    expect(await screen.findByRole("heading", { name: "Sign-in is the next step." })).toBeTruthy();
    expect(window.location.hash).toBe("#/onboarding");
  });

  it("leaves the workspace pages and their navigation intact", async () => {
    const user = userEvent.setup();
    render(<App />);
    goto("#/home");

    await screen.findByRole("heading", { name: "Welcome to Nexora" });
    await user.click(screen.getByRole("link", { name: "AI Chat" }));
    expect(await screen.findByRole("region", { name: "Conversation messages" })).toBeTruthy();

    await user.click(screen.getByRole("link", { name: "Projects" }));
    expect(await screen.findByRole("complementary", { name: "Project explorer" })).toBeTruthy();
  });
});
