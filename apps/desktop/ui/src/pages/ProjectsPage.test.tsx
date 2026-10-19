import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ProjectsPage } from "./ProjectsPage";
import { makeStatus } from "../test/fixtures";

function makeWorkspaceMock(overrides?: Partial<Record<string, unknown>>) {
  return {
    workspacePath: "C:\\projects\\my-app",
    workspaceName: "my-app",
    inventory: {
      root: "C:\\projects\\my-app",
      entries: [
        { path: "src/index.ts", kind: "file", sizeBytes: 1024 },
        { path: "src/utils.ts", kind: "file", sizeBytes: 512 },
      ],
      stats: { fileCount: 2, dirCount: 1, totalBytes: 1536 },
      truncated: false,
    },
    loading: false,
    error: null,
    search: "",
    setSearch: vi.fn(),
    filteredEntries: [
      { path: "src/index.ts", kind: "file", sizeBytes: 1024 },
      { path: "src/utils.ts", kind: "file", sizeBytes: 512 },
    ],
    selectedPaths: new Set<string>(),
    contextFiles: [],
    contextError: null,
    previewPath: null,
    preview: null,
    previewLoading: false,
    previewError: null,
    openWorkspace: vi.fn().mockResolvedValue(null),
    refresh: vi.fn().mockResolvedValue(undefined),
    closeWorkspace: vi.fn().mockResolvedValue(undefined),
    loadPreview: vi.fn().mockResolvedValue(undefined),
    clearPreview: vi.fn(),
    toggleContext: vi.fn(),
    clearContext: vi.fn(),
    ...overrides,
  } as unknown as Parameters<typeof ProjectsPage>[0]["workspace"];
}

function makePatchMock(overrides?: Partial<Record<string, unknown>>) {
  return {
    patchState: { status: "idle" as const },
    editorContent: "",
    startEditing: vi.fn(),
    updateEditor: vi.fn(),
    cancel: vi.fn(),
    createProposal: vi.fn().mockResolvedValue(undefined),
    apply: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  } as unknown as Parameters<typeof ProjectsPage>[0]["patch"];
}

function makeValidationMock() {
  return {
    validationState: { status: "idle" as const },
    validate: vi.fn().mockResolvedValue(undefined),
    cancel: vi.fn(),
    reset: vi.fn(),
  } as unknown as Parameters<typeof ProjectsPage>[0]["validation"];
}

function makeRepairMock() {
  return {
    repairState: { status: "idle" as const },
    repair: vi.fn().mockResolvedValue(undefined),
    explain: vi.fn().mockResolvedValue(undefined),
    cancel: vi.fn(),
    reset: vi.fn(),
    maxAttempts: 3,
  } as unknown as Parameters<typeof ProjectsPage>[0]["repair"];
}

describe("Phase 26 — ProjectsPage component", () => {
  it("renders empty state call to action when no project is loaded", async () => {
    const workspace = makeWorkspaceMock({
      workspacePath: null,
      workspaceName: null,
      inventory: null,
      filteredEntries: [],
    });
    const patch = makePatchMock();
    const validation = makeValidationMock();
    const repair = makeRepairMock();
    const user = userEvent.setup();

    render(
      <ProjectsPage
        workspace={workspace}
        patch={patch}
        validation={validation}
        repair={repair}
        evidenceReport={null}
        patchMissing={null}
        generationTargetPath={null}
        generationOriginalContent={null}
        groundedContextBlock={null}
        engineStatus={makeStatus()}
        symbolIndex={null}
        isStreaming={false}
      />,
    );

    expect(screen.getByRole("heading", { name: "No project open" })).toBeTruthy();
    expect(screen.getByText(/Open a project directory to browse files/i)).toBeTruthy();

    const openBtns = screen.getAllByRole("button", { name: /Open project/i });
    expect(openBtns.length).toBeGreaterThanOrEqual(1);

    await user.click(openBtns[0]!);
    expect(workspace.openWorkspace).toHaveBeenCalled();
  });

  it("renders active project header, path, and action buttons when workspace is open", async () => {
    const workspace = makeWorkspaceMock();
    const patch = makePatchMock();
    const validation = makeValidationMock();
    const repair = makeRepairMock();
    const user = userEvent.setup();

    render(
      <ProjectsPage
        workspace={workspace}
        patch={patch}
        validation={validation}
        repair={repair}
        evidenceReport={null}
        patchMissing={null}
        generationTargetPath={null}
        generationOriginalContent={null}
        groundedContextBlock={null}
        engineStatus={makeStatus()}
        symbolIndex={null}
        isStreaming={false}
      />,
    );

    expect(screen.getByRole("heading", { name: "my-app" })).toBeTruthy();
    expect(screen.getAllByText("C:\\projects\\my-app").length).toBeGreaterThanOrEqual(1);

    const switchBtns = screen.getAllByRole("button", { name: "Switch project" });
    const refreshBtn = screen.getByRole("button", { name: "Refresh" });
    const closeBtn = screen.getByRole("button", { name: "Close" });

    await user.click(switchBtns[0]!);
    expect(workspace.openWorkspace).toHaveBeenCalled();

    await user.click(refreshBtn);
    expect(workspace.refresh).toHaveBeenCalled();

    await user.click(closeBtn);
    expect(workspace.closeWorkspace).toHaveBeenCalled();
  });

  it("renders file preview content when a file is selected", () => {
    const workspace = makeWorkspaceMock({
      previewPath: "src/index.ts",
      preview: {
        path: "src/index.ts",
        content: "export const greeting = 'Hello, Nexora!';",
        size: 42,
      },
    });
    const patch = makePatchMock();
    const validation = makeValidationMock();
    const repair = makeRepairMock();

    render(
      <ProjectsPage
        workspace={workspace}
        patch={patch}
        validation={validation}
        repair={repair}
        evidenceReport={null}
        patchMissing={null}
        generationTargetPath={null}
        generationOriginalContent={null}
        groundedContextBlock={null}
        engineStatus={makeStatus()}
        symbolIndex={null}
        isStreaming={false}
      />,
    );

    expect(screen.getByText("export const greeting = 'Hello, Nexora!';")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Propose change" })).toBeTruthy();
  });
});
