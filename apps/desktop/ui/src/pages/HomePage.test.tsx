import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mockGetWorkspaceSettings = vi.fn();

vi.mock("../lib/workspaceSettings", () => ({
  getWorkspaceSettings: () => mockGetWorkspaceSettings(),
}));

import { HomePage } from "./HomePage";
import { makeStatus } from "../test/fixtures";

describe("Phase 24 — Dashboard / HomePage", () => {
  beforeEach(() => {
    mockGetWorkspaceSettings.mockReset();
    mockGetWorkspaceSettings.mockResolvedValue({
      lastWorkspacePath: null,
      preferences: {},
    });
  });

  it("renders the primary dashboard quick actions and title", () => {
    const onNewChat = vi.fn();
    render(
      <HomePage
        status={null}
        statusError={null}
        engineReady={false}
        workspacePath={null}
        workspaceName={null}
        conversationCount={0}
        onNewChat={onNewChat}
      />,
    );

    expect(screen.getByRole("heading", { name: "Welcome to Nexora" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Start a new chat/i })).toBeTruthy();
    expect(screen.getByRole("link", { name: /Projects Open a folder/i })).toBeTruthy();
    expect(screen.getByRole("link", { name: /Code intelligence Search/i })).toBeTruthy();
    expect(screen.getByRole("link", { name: /Local models Connect/i })).toBeTruthy();
  });

  it("displays genuine local AI engine readiness and active model details", () => {
    const status = makeStatus({
      state: "ready",
      modelPath: "C:\\models\\qwen2.5-coder-7b.gguf",
      contextSize: 32768,
    });

    render(
      <HomePage
        status={status}
        statusError={null}
        engineReady={true}
        workspacePath={null}
        workspaceName={null}
        conversationCount={0}
        onNewChat={vi.fn()}
      />,
    );

    expect(screen.getByText("Ready")).toBeTruthy();
    expect(screen.getByText(/Active model: qwen2\.5-coder-7b\.gguf/i)).toBeTruthy();
    expect(screen.getByText(/32768 tokens/i)).toBeTruthy();
    expect(screen.getByRole("link", { name: "Model settings" })).toBeTruthy();
  });

  it("displays project information when a workspace is open", () => {
    render(
      <HomePage
        status={null}
        statusError={null}
        engineReady={false}
        workspacePath="C:\\Projects\\NexoraApp"
        workspaceName="NexoraApp"
        fileCount={42}
        selectedFileCount={3}
        symbolCount={120}
        relationshipCount={28}
        conversationCount={2}
        onNewChat={vi.fn()}
      />,
    );

    expect(screen.getAllByText(/NexoraApp/i).length).toBe(2);
    expect(screen.getByText(/42 files in inventory/i)).toBeTruthy();
    expect(screen.getByText(/3 selected for context/i)).toBeTruthy();
    expect(screen.getByText("120 symbols")).toBeTruthy();
    expect(screen.getByText(/28 dependency edges mapped/i)).toBeTruthy();
    expect(screen.getByRole("link", { name: "Manage project" })).toBeTruthy();
  });

  it("shows clear empty states and allows opening a project or resuming recent", async () => {
    const user = userEvent.setup();
    mockGetWorkspaceSettings.mockResolvedValue({
      lastWorkspacePath: "D:\\Dev\\PriorRepo",
      preferences: {},
    });

    const onOpenProject = vi.fn();
    const onOpenPath = vi.fn().mockResolvedValue(undefined);

    render(
      <HomePage
        status={null}
        statusError={null}
        engineReady={false}
        workspacePath={null}
        workspaceName={null}
        conversationCount={0}
        onNewChat={vi.fn()}
        onOpenProject={onOpenProject}
        onOpenPath={onOpenPath}
      />,
    );

    expect(screen.getByText("No project open")).toBeTruthy();

    // Open folder action
    const openBtn = screen.getByRole("button", { name: /Open folder/i });
    await user.click(openBtn);
    expect(onOpenProject).toHaveBeenCalled();

    // Reopen recent action
    const resumeBtn = await screen.findByRole("button", { name: /Reopen recent/i });
    await user.click(resumeBtn);
    expect(onOpenPath).toHaveBeenCalledWith("D:\\Dev\\PriorRepo");
  });

  it("supports manual workspace path entry", async () => {
    const user = userEvent.setup();
    const onOpenPath = vi.fn().mockResolvedValue(undefined);

    render(
      <HomePage
        status={null}
        statusError={null}
        engineReady={false}
        workspacePath={null}
        workspaceName={null}
        conversationCount={0}
        onNewChat={vi.fn()}
        onOpenPath={onOpenPath}
      />,
    );

    const input = screen.getByPlaceholderText(/Enter absolute directory path/i);
    const submitBtn = screen.getByRole("button", { name: /Open path/i });

    await user.type(input, "E:\\Code\\MyApp");
    await user.click(submitBtn);

    await waitFor(() => {
      expect(onOpenPath).toHaveBeenCalledWith("E:\\Code\\MyApp");
    });
  });

  it("lists recent conversations with one-click continuation", async () => {
    const user = userEvent.setup();
    const onSelectConversation = vi.fn();

    const recent = [
      { id: "conv-1", title: "Refactor Rust IPC", updatedAt: Date.now() - 60000 },
      { id: "conv-2", title: "Add Dashboard tests", updatedAt: Date.now() - 3600000 },
    ];

    render(
      <HomePage
        status={null}
        statusError={null}
        engineReady={false}
        workspacePath={null}
        workspaceName={null}
        conversationCount={2}
        recentConversations={recent}
        onNewChat={vi.fn()}
        onSelectConversation={onSelectConversation}
      />,
    );

    expect(screen.getByText("2 saved")).toBeTruthy();
    expect(screen.getByText("Refactor Rust IPC")).toBeTruthy();
    expect(screen.getByText("Add Dashboard tests")).toBeTruthy();

    const convBtn = screen.getByRole("button", { name: /Refactor Rust IPC/i });
    await user.click(convBtn);
    expect(onSelectConversation).toHaveBeenCalledWith("conv-1");
  });
});
