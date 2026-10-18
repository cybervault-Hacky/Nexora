import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { WorkspacePanel } from "./WorkspacePanel";
import type { WorkspaceInventory } from "../lib/workspace";

function makeInventory(entries: WorkspaceInventory["entries"] = []): WorkspaceInventory {
  return {
    rootName: "my-app",
    rootPath: "/tmp/my-app",
    entries,
    truncated: false,
    stats: { fileCount: entries.filter((e) => e.kind === "file").length, dirCount: entries.filter((e) => e.kind === "dir").length, skippedExcluded: 0, skippedSensitive: 0, skippedOversized: 0, skippedUnreadable: 0, skippedSymlinkOutside: 0, skippedDepth: 0 },
    limits: { maxDepth: 12, maxFiles: 8000, maxPreviewBytes: 262144, maxInventoryBytes: 2097152 },
  };
}

const baseEntries: WorkspaceInventory["entries"] = [
  { path: "src", kind: "dir", size: 0, depth: 1 },
  { path: "src/main.rs", kind: "file", size: 120, depth: 1 },
  { path: "README.md", kind: "file", size: 50, depth: 0 },
];

describe("WorkspacePanel", () => {
  it("shows empty state and open button when no workspace", async () => {
    const props = {
      workspacePath: null,
      workspaceName: null,
      inventory: null,
      loading: false,
      error: null,
      search: "",
      onSearchChange: vi.fn(),
      filteredEntries: [],
      selectedPaths: new Set<string>(),
      onToggleContext: vi.fn(),
      onClearContext: vi.fn(),
      contextError: null,
      onOpen: vi.fn(),
      onRefresh: vi.fn(),
      onClose: vi.fn(),
      onPreview: vi.fn(),
      previewPath: null,
    };
    render(<WorkspacePanel {...props} />);
    expect(screen.getByText(/Open a local folder/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Open project" })).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Open project" }));
    expect(props.onOpen).toHaveBeenCalled();
  });

  it("displays workspace name, path, and controls when open", () => {
    const inv = makeInventory(baseEntries);
    const props = {
      workspacePath: "/tmp/my-app",
      workspaceName: "my-app",
      inventory: inv,
      loading: false,
      error: null,
      search: "",
      onSearchChange: vi.fn(),
      filteredEntries: baseEntries,
      selectedPaths: new Set<string>(),
      onToggleContext: vi.fn(),
      onClearContext: vi.fn(),
      contextError: null,
      onOpen: vi.fn(),
      onRefresh: vi.fn(),
      onClose: vi.fn(),
      onPreview: vi.fn(),
      previewPath: null,
    };
    render(<WorkspacePanel {...props} />);
    expect(screen.getByText("my-app")).toBeTruthy();
    expect(screen.getByText("/tmp/my-app")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Refresh project" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Close project" })).toBeTruthy();
  });

  it("renders searchable file inventory with dir/file distinction", async () => {
    const inv = makeInventory(baseEntries);
    const props = {
      workspacePath: "/tmp/my-app",
      workspaceName: "my-app",
      inventory: inv,
      loading: false,
      error: null,
      search: "",
      onSearchChange: vi.fn(),
      filteredEntries: baseEntries,
      selectedPaths: new Set<string>(),
      onToggleContext: vi.fn(),
      onClearContext: vi.fn(),
      contextError: null,
      onOpen: vi.fn(),
      onRefresh: vi.fn(),
      onClose: vi.fn(),
      onPreview: vi.fn(),
      previewPath: null,
    };
    const { rerender } = render(<WorkspacePanel {...props} />);
    const list = screen.getByRole("list", { name: "Project files" });
    const items = within(list).getAllByRole("listitem");
    expect(items.length).toBe(3);
    // dir should be disabled preview
    expect((screen.getByRole("button", { name: "src directory" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole("button", { name: "Preview src/main.rs" })).toBeTruthy();

    // search filtering
    expect(screen.getByRole("search", { name: "Project file paths" })).toBeTruthy();
    const searchInput = screen.getByRole("textbox", { name: "Filter project paths" });
    expect(searchInput.getAttribute("placeholder")).toBe("Filter project paths…");
    await userEvent.type(searchInput, "README");
    expect(props.onSearchChange).toHaveBeenCalled();

    // Filtering remains a live relative-path match, with an explicit clear action.
    rerender(<WorkspacePanel {...props} filteredEntries={baseEntries.filter((e) => e.path.includes("README"))} search="README" />);
    expect(screen.getByRole("button", { name: "Preview README.md" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Preview src/main.rs" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Clear path filter" }));
    expect(props.onSearchChange).toHaveBeenLastCalledWith("");
  });

  it("handles loading, empty, error, and truncated states", () => {
    const loadingProps = {
      workspacePath: "/tmp/my-app",
      workspaceName: "my-app",
      inventory: makeInventory([]),
      loading: true,
      error: null,
      search: "",
      onSearchChange: vi.fn(),
      filteredEntries: [],
      selectedPaths: new Set<string>(),
      onToggleContext: vi.fn(),
      onClearContext: vi.fn(),
      contextError: null,
      onOpen: vi.fn(),
      onRefresh: vi.fn(),
      onClose: vi.fn(),
      onPreview: vi.fn(),
      previewPath: null,
    };
    const { rerender } = render(<WorkspacePanel {...loadingProps} />);
    expect(screen.getByRole("status").textContent).toContain("Scanning");

    const emptyProps = { ...loadingProps, loading: false, filteredEntries: [], inventory: makeInventory([]) };
    rerender(<WorkspacePanel {...emptyProps} />);
    expect(screen.getByText(/No files to display/)).toBeTruthy();
    rerender(<WorkspacePanel {...emptyProps} search="unmatched" />);
    expect(screen.getByRole("status").textContent).toContain("No paths match");

    const errorProps = { ...emptyProps, error: "Project directory not found" };
    rerender(<WorkspacePanel {...errorProps} />);
    expect(screen.getByRole("alert").textContent).toContain("Project directory not found");
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();

    const truncatedProps = {
      ...emptyProps,
      inventory: { ...makeInventory(baseEntries), truncated: true, limits: { maxDepth: 12, maxFiles: 8000, maxPreviewBytes: 262144, maxInventoryBytes: 2097152 } },
      filteredEntries: baseEntries,
    };
    rerender(<WorkspacePanel {...truncatedProps} />);
    expect(screen.getByText(/Results truncated/)).toBeTruthy();
  });

  it("toggles context and shows selected state", async () => {
    const inv = makeInventory(baseEntries);
    const onToggle = vi.fn();
    const props = {
      workspacePath: "/tmp/my-app",
      workspaceName: "my-app",
      inventory: inv,
      loading: false,
      error: null,
      search: "",
      onSearchChange: vi.fn(),
      filteredEntries: baseEntries,
      selectedPaths: new Set<string>(["src/main.rs"]),
      onToggleContext: onToggle,
      onClearContext: vi.fn(),
      contextError: null,
      onOpen: vi.fn(),
      onRefresh: vi.fn(),
      onClose: vi.fn(),
      onPreview: vi.fn(),
      previewPath: null,
    };
    render(<WorkspacePanel {...props} />);
    const checkbox = screen.getByRole("checkbox", { name: "Include src/main.rs as context" }) as HTMLInputElement;
    expect(checkbox.checked).toBe(true);
    await userEvent.click(checkbox);
    expect(onToggle).toHaveBeenCalledWith("src/main.rs");

    // not selected file
    const other = screen.getByRole("checkbox", { name: "Include README.md as context" }) as HTMLInputElement;
    expect(other.checked).toBe(false);
  });

  it("previews file on click", async () => {
    const inv = makeInventory(baseEntries);
    const onPreview = vi.fn();
    const props = {
      workspacePath: "/tmp/my-app",
      workspaceName: "my-app",
      inventory: inv,
      loading: false,
      error: null,
      search: "",
      onSearchChange: vi.fn(),
      filteredEntries: baseEntries,
      selectedPaths: new Set<string>(),
      onToggleContext: vi.fn(),
      onClearContext: vi.fn(),
      contextError: null,
      onOpen: vi.fn(),
      onRefresh: vi.fn(),
      onClose: vi.fn(),
      onPreview,
      previewPath: null,
    };
    render(<WorkspacePanel {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "Preview src/main.rs" }));
    expect(onPreview).toHaveBeenCalledWith("src/main.rs");
  });

  it("shows context errors and stats", () => {
    const inv = makeInventory(baseEntries);
    inv.stats.skippedExcluded = 3;
    inv.stats.skippedSymlinkOutside = 2;
    const props = {
      workspacePath: "/tmp/my-app",
      workspaceName: "my-app",
      inventory: inv,
      loading: false,
      error: null,
      search: "",
      onSearchChange: vi.fn(),
      filteredEntries: baseEntries,
      selectedPaths: new Set<string>(),
      onToggleContext: vi.fn(),
      onClearContext: vi.fn(),
      contextError: "Selected files exceed the 100 KiB context limit.",
      onOpen: vi.fn(),
      onRefresh: vi.fn(),
      onClose: vi.fn(),
      onPreview: vi.fn(),
      previewPath: null,
    };
    render(<WorkspacePanel {...props} />);
    expect(screen.getByText(/exceed/)).toBeTruthy();
    expect(screen.getByText(/3 excluded/)).toBeTruthy();
  });

  it("respects keyboard accessibility", async () => {
    const inv = makeInventory(baseEntries);
    const props = {
      workspacePath: "/tmp/my-app",
      workspaceName: "my-app",
      inventory: inv,
      loading: false,
      error: null,
      search: "",
      onSearchChange: vi.fn(),
      filteredEntries: baseEntries,
      selectedPaths: new Set<string>(),
      onToggleContext: vi.fn(),
      onClearContext: vi.fn(),
      contextError: null,
      onOpen: vi.fn(),
      onRefresh: vi.fn(),
      onClose: vi.fn(),
      onPreview: vi.fn(),
      previewPath: "src/main.rs",
    };
    render(<WorkspacePanel {...props} />);
    const btn = screen.getByRole("button", { name: "Preview src/main.rs" });
    btn.focus();
    expect(document.activeElement).toBe(btn);
  });
});
