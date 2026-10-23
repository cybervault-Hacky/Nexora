import { createRef } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ChatPage } from "./ChatPage";
import type { ChatMessage } from "../features/chat/chat";
import { makeReadiness, makeStatus } from "../test/fixtures";
import type { UsageView } from "../features/usage/useUsage";
import type { ModelCatalog } from "../features/models/useModelCatalog";

function makeUsageMock(overrides: Partial<UsageView> = {}): UsageView {
  return {
    status: null,
    error: null,
    loading: false,
    reload: vi.fn(),
    now: Date.UTC(2026, 9, 20, 12),
    ...overrides,
  };
}

/** A catalogue with both profiles, Mini selected and ready. */
function makeModelsMock(overrides: Partial<ModelCatalog> = {}): ModelCatalog {
  const mini = {
    profile: {
      id: "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m",
      displayName: "Nexora Mini",
      publisher: "Qwen team, Alibaba Group",
      upstreamModel: "Qwen/Qwen2.5-Coder-7B-Instruct",
      upstreamRevision: "c03e6d358207e414f1eca0bb1891e29f1db0e242",
      format: "GGUF",
      quantization: "Q4_K_M",
      licence: { id: "Apache-2.0", reference: "https://example.invalid/LICENSE" },
      artifact: {
        state: "approved" as const,
        repository: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF",
        revision: "13fb94bfda8c8cf22497dc57b78f391a9acb426a",
        fileName: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
        sizeBytes: 4_683_073_536,
        sha256: "509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c",
      },
      context: { recommended: 32_768, documentedMax: 32_768 },
      compatibilityNotes: ["note"],
      locallyVerified: false,
    },
    selected: true,
    active: true,
    installedPath: "C:\\Models\\qwen2.5-coder-7b-instruct-q4_k_m.gguf",
    downloadable: true,
    status: "loaded" as const,
    detail: "This model is loaded and answering from this machine.",
    readiness: makeReadiness({
      state: "loaded",
      headline: "Loaded",
      detail: "This model is loaded and answering from this machine.",
      canDownload: false,
    }),
  };

  const sonnet = {
    ...mini,
    profile: {
      ...mini.profile,
      id: "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m",
      displayName: "Nexora Sonnet",
      upstreamModel: "Qwen/Qwen2.5-Coder-14B-Instruct",
      upstreamRevision: "aedcc2d42b622764e023cf882b6652e646b95671",
      artifact: {
        state: "approved" as const,
        repository: "Qwen/Qwen2.5-Coder-14B-Instruct-GGUF",
        revision: "d0a692ef765eefbf2fabb130b3cb2e8917e3d225",
        fileName: "qwen2.5-coder-14b-instruct-q4_k_m.gguf",
        sizeBytes: 8_988_110_272,
        sha256: "c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940",
      },
      context: { recommended: 8_192, documentedMax: 32_768 },
    },
    selected: false,
    active: false,
    installedPath: null,
    downloadable: true,
    status: "ready_to_download" as const,
    detail: "Nothing is installed yet.",
    readiness: makeReadiness(),
  };

  return {
    entries: [mini, sonnet],
    selected: mini,
    running: true,
    loadedProfileId: mini.profile.id,
    selectionError: null,
    loading: false,
    error: null,
    selecting: false,
    switchError: null,
    dismissSwitchError: vi.fn(),
    select: vi.fn(),
    reload: vi.fn(),
    ...overrides,
  };
}

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
    canRetry: false,
    setDraft: vi.fn(),
    dismissError: vi.fn(),
    dismissRuntimeError: vi.fn(),
    dismissSaveError: vi.fn(),
    dismissLoadError: vi.fn(),
    refreshList: vi.fn(),
    submit: vi.fn(),
    retry: vi.fn(),
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
  it("renders no usage meter in the browser preview", () => {
    const textareaRef = createRef<HTMLTextAreaElement>();

    render(
      <ChatPage
        conversation={makeConversationMock()}
        workspace={makeWorkspaceMock()}
        engineStatus={makeStatus()}
        statusError={null}
        engineReady={true}
        desktop={false}
        contextPlan={null}
        retrievalResult={null}
        evidenceReport={null}
        textareaRef={textareaRef}
        focusNonce={0}
        onSubmit={vi.fn()}
        usage={makeUsageMock()}
        models={makeModelsMock()}
      />,
    );

    expect(screen.queryByLabelText("Daily free usage")).toBeNull();
    expect(screen.getByRole("button", { name: "Send message" })).toBeTruthy();
  });

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
        usage={makeUsageMock()}
        models={makeModelsMock()}
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
        usage={makeUsageMock()}
        models={makeModelsMock()}
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
        usage={makeUsageMock()}
        models={makeModelsMock()}
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
        usage={makeUsageMock()}
        models={makeModelsMock()}
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
        usage={makeUsageMock()}
        models={makeModelsMock()}
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
        usage={makeUsageMock()}
        models={makeModelsMock()}
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
        usage={makeUsageMock()}
        models={makeModelsMock()}
      />,
    );

    const newChatBtn = screen.getByRole("button", { name: "Start new chat" });
    await user.click(newChatBtn);

    expect(conversation.newChat).toHaveBeenCalled();
  });
});
