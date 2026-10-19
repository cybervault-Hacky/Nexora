import { createRef } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ChatPage } from "./ChatPage";
import type { ChatMessage } from "../features/chat/chat";
import { makeStatus } from "../test/fixtures";

function makeConversationMock(overrides?: Partial<Record<string, unknown>>) {
  return {
    conversations: [],
    activeId: null,
    activeTitle: "New conversation",
    listLoading: false,
    listError: null,
    saveStatus: "idle" as const,
    saveError: null,
    loadError: null,
    conversationLoading: false,
    messages: [] as ChatMessage[],
    draft: "",
    error: null,
    runtimeError: null,
    streaming: false,
    setDraft: vi.fn(),
    dismissError: vi.fn(),
    dismissRuntimeError: vi.fn(),
    dismissSaveError: vi.fn(),
    dismissLoadError: vi.fn(),
    refreshList: vi.fn(),
    submit: vi.fn(),
    stop: vi.fn(),
    newChat: vi.fn(),
    selectConversation: vi.fn(),
    renameActive: vi.fn().mockResolvedValue(undefined),
    renameById: vi.fn().mockResolvedValue(undefined),
    deleteActive: vi.fn().mockResolvedValue(undefined),
    deleteConversationById: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  } as unknown as Parameters<typeof ChatPage>[0]["conversation"];
}

function makeWorkspaceMock(overrides?: Partial<Record<string, unknown>>) {
  return {
    workspacePath: "C:\\projects\\my-app",
    previewPath: null,
    inventory: {
      root: "C:\\projects\\my-app",
      entries: [],
      stats: { fileCount: 12, directoryCount: 3, totalBytes: 40960 },
      truncated: false,
    },
    loading: false,
    scanning: false,
    error: null,
    openWorkspace: vi.fn(),
    closeWorkspace: vi.fn(),
    loadPreview: vi.fn(),
    selectedPaths: new Set<string>(),
    contextFiles: [],
    contextBlock: "",
    toggleContext: vi.fn(),
    clearContext: vi.fn(),
    clearAll: vi.fn(),
    ...overrides,
  } as unknown as Parameters<typeof ChatPage>[0]["workspace"];
}

describe("Phase 25 — ChatPage component", () => {
  it("renders conversation header, engine status chip, and local-only chip", () => {
    const conversation = makeConversationMock();
    const workspace = makeWorkspaceMock();
    const textareaRef = createRef<HTMLTextAreaElement>();

    render(
      <ChatPage
        conversation={conversation}
        workspace={workspace}
        engineStatus={makeStatus()}
        statusError={null}
        engineReady={true}
        desktop={true}
        contextPlan={null}
        retrievalResult={null}
        evidenceReport={null}
        textareaRef={textareaRef}
        focusNonce={0}
        onSubmit={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "New conversation" })).toBeTruthy();
    expect(screen.getByText("Local-only")).toBeTruthy();
    expect(screen.getByText("Engine ready")).toBeTruthy();
  });

  it("renders EmptyState with suggestion prompts when engine is ready and there are no messages", async () => {
    const conversation = makeConversationMock();
    const workspace = makeWorkspaceMock();
    const textareaRef = createRef<HTMLTextAreaElement>();
    const user = userEvent.setup();

    render(
      <ChatPage
        conversation={conversation}
        workspace={workspace}
        engineStatus={makeStatus()}
        statusError={null}
        engineReady={true}
        desktop={true}
        contextPlan={null}
        retrievalResult={null}
        evidenceReport={null}
        textareaRef={textareaRef}
        focusNonce={0}
        onSubmit={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Build something real." })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Build a landing page/i })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: /Build a landing page/i }));
    expect(conversation.setDraft).toHaveBeenCalledWith(
      expect.stringContaining("Build a responsive landing page"),
    );
  });

  it("renders 'Bring your own model' empty state when no model is loaded", () => {
    const conversation = makeConversationMock();
    const workspace = makeWorkspaceMock();
    const textareaRef = createRef<HTMLTextAreaElement>();

    render(
      <ChatPage
        conversation={conversation}
        workspace={workspace}
        engineStatus={makeStatus({ state: "stopped", modelPath: null })}
        statusError={null}
        engineReady={false}
        desktop={true}
        contextPlan={null}
        retrievalResult={null}
        evidenceReport={null}
        textareaRef={textareaRef}
        focusNonce={0}
        onSubmit={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Bring your own model." })).toBeTruthy();
    expect(screen.getByRole("link", { name: /Open local model manager/i })).toBeTruthy();
  });

  it("renders user prompt and assistant reply when messages exist", () => {
    const messages: ChatMessage[] = [
      {
        id: "msg-1",
        role: "user",
        content: "What is React 19?",
        createdAt: Date.now() - 60000,
        state: "complete",
      },
      {
        id: "msg-2",
        role: "assistant",
        content: "React 19 includes Actions, Server Components, and new hooks.",
        createdAt: Date.now() - 30000,
        state: "complete",
      },
    ];

    const conversation = makeConversationMock({
      messages,
      activeId: "chat-1",
      activeTitle: "What is React 19?",
    });
    const workspace = makeWorkspaceMock();
    const textareaRef = createRef<HTMLTextAreaElement>();

    render(
      <ChatPage
        conversation={conversation}
        workspace={workspace}
        engineStatus={makeStatus()}
        statusError={null}
        engineReady={true}
        desktop={true}
        contextPlan={null}
        retrievalResult={null}
        evidenceReport={null}
        textareaRef={textareaRef}
        focusNonce={0}
        onSubmit={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "What is React 19?" })).toBeTruthy();
    expect(document.querySelector(".message__prompt")?.textContent).toBe("What is React 19?");
    expect(screen.getByText(/React 19 includes Actions/)).toBeTruthy();
  });

  it("renders selected workspace context files and allows clearing them", async () => {
    const workspace = makeWorkspaceMock({
      contextFiles: [
        { path: "src/main.ts", sizeBytes: 1024 },
        { path: "src/utils.ts", sizeBytes: 512 },
      ],
    });
    const conversation = makeConversationMock();
    const textareaRef = createRef<HTMLTextAreaElement>();
    const user = userEvent.setup();

    render(
      <ChatPage
        conversation={conversation}
        workspace={workspace}
        engineStatus={makeStatus()}
        statusError={null}
        engineReady={true}
        desktop={true}
        contextPlan={null}
        retrievalResult={null}
        evidenceReport={null}
        textareaRef={textareaRef}
        focusNonce={0}
        onSubmit={vi.fn()}
      />,
    );

    expect(screen.getByText(/Context: 2 files — read-only, not modifying files/)).toBeTruthy();
    expect(screen.getByText("src/main.ts")).toBeTruthy();
    expect(screen.getByText("src/utils.ts")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Clear" }));
    expect(workspace.clearContext).toHaveBeenCalled();
  });

  it("handles conversation renaming with Save button", async () => {
    const conversation = makeConversationMock({
      activeId: "chat-1",
      activeTitle: "Initial Title",
      messages: [{ id: "m1", role: "user", content: "hi", createdAt: Date.now(), state: "complete" }],
    });
    const workspace = makeWorkspaceMock();
    const textareaRef = createRef<HTMLTextAreaElement>();
    const user = userEvent.setup();

    render(
      <ChatPage
        conversation={conversation}
        workspace={workspace}
        engineStatus={makeStatus()}
        statusError={null}
        engineReady={true}
        desktop={true}
        contextPlan={null}
        retrievalResult={null}
        evidenceReport={null}
        textareaRef={textareaRef}
        focusNonce={0}
        onSubmit={vi.fn()}
      />,
    );

    const renameBtn = screen.getByRole("button", { name: "Rename conversation" });
    await user.click(renameBtn);

    const renameInput = screen.getByLabelText("Conversation title");
    await user.clear(renameInput);
    await user.type(renameInput, "Updated Project Discussion");

    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(conversation.renameActive).toHaveBeenCalledWith("Updated Project Discussion");
  });

  it("renders New Chat button in header when conversation has messages and invokes newChat", async () => {
    const conversation = makeConversationMock({
      activeId: "chat-1",
      activeTitle: "Ongoing Chat",
      messages: [{ id: "m1", role: "user", content: "test", createdAt: Date.now(), state: "complete" }],
    });
    const workspace = makeWorkspaceMock();
    const textareaRef = createRef<HTMLTextAreaElement>();
    const user = userEvent.setup();

    render(
      <ChatPage
        conversation={conversation}
        workspace={workspace}
        engineStatus={makeStatus()}
        statusError={null}
        engineReady={true}
        desktop={true}
        contextPlan={null}
        retrievalResult={null}
        evidenceReport={null}
        textareaRef={textareaRef}
        focusNonce={0}
        onSubmit={vi.fn()}
      />,
    );

    const newChatBtn = screen.getByRole("button", { name: "Start new chat" });
    await user.click(newChatBtn);

    expect(conversation.newChat).toHaveBeenCalled();
  });
});
