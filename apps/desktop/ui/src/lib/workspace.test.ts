import { invoke } from "@tauri-apps/api/core";
import { markDesktop } from "../test/fixtures";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  clearStoredWorkspace,
  scanWorkspace,
  readWorkspaceFile,
  getWorkspaceContext,
  setWorkspace,
} from "./workspace";

describe("workspace lib desktop boundary", () => {
  beforeEach(() => {
    markDesktop(true);
    vi.mocked(invoke).mockReset();
  });
  afterEach(() => {
    markDesktop(false);
    clearStoredWorkspace();
    localStorage.clear();
  });

  it("maps workspace errors without leaking paths", async () => {
    vi.mocked(invoke).mockRejectedValue({ code: "forbidden", message: "File escapes workspace." });
    await expect(readWorkspaceFile("/root", "../etc/passwd")).rejects.toMatchObject({ code: "forbidden" });
  });

  it("scans workspace via invoke", async () => {
    const inventory = {
      rootName: "my-app",
      rootPath: "/tmp/my-app",
      entries: [
        { path: "src/main.rs", kind: "file", size: 120, depth: 1 },
        { path: "src", kind: "dir", size: 0, depth: 1 },
      ],
      truncated: false,
      stats: { fileCount: 1, dirCount: 1, skippedExcluded: 1, skippedSensitive: 0, skippedOversized: 0, skippedUnreadable: 0, skippedSymlinkOutside: 0, skippedDepth: 0 },
      limits: { maxDepth: 12, maxFiles: 8000, maxPreviewBytes: 262144, maxInventoryBytes: 2097152 },
    };
    vi.mocked(invoke).mockResolvedValue(inventory);
    const result = await scanWorkspace("/tmp/my-app");
    expect(result.rootName).toBe("my-app");
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("scan_workspace_command", { root: "/tmp/my-app" });
  });

  it("reads file via invoke with relative_path mapping", async () => {
    const preview = { path: "src/main.rs", content: "fn main(){}", size: 12, truncated: false, encoding: "utf8" };
    vi.mocked(invoke).mockResolvedValue(preview);
    const result = await readWorkspaceFile("/tmp/my-app", "src/main.rs");
    expect(result.content).toContain("fn main");
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("read_workspace_file_command", { root: "/tmp/my-app", relative_path: "src/main.rs" });
  });

  it("rejects traversal via backend error", async () => {
    vi.mocked(invoke).mockRejectedValue({ code: "invalid_request", message: "Path traversal is not allowed." });
    await expect(readWorkspaceFile("/tmp/my-app", "../outside.txt")).rejects.toMatchObject({ code: "invalid_request" });
  });

  it("rejects sensitive files", async () => {
    vi.mocked(invoke).mockRejectedValue({ code: "secret_excluded", message: "This file is excluded" });
    await expect(readWorkspaceFile("/tmp/my-app", ".env")).rejects.toMatchObject({ code: "secret_excluded" });
    await expect(getWorkspaceContext("/tmp/my-app", [".env"])).rejects.toMatchObject({ code: "secret_excluded" });
  });

  it("rejects binary files", async () => {
    vi.mocked(invoke).mockRejectedValue({ code: "binary", message: "Binary files cannot be previewed" });
    await expect(readWorkspaceFile("/tmp/my-app", "image.png")).rejects.toMatchObject({ code: "binary" });
  });

  it("rejects oversized files", async () => {
    vi.mocked(invoke).mockRejectedValue({ code: "too_large", message: "File is too large" });
    await expect(readWorkspaceFile("/tmp/my-app", "big.log")).rejects.toMatchObject({ code: "too_large" });
  });

  it("enforces context limits", async () => {
    vi.mocked(invoke).mockRejectedValue({ code: "too_large", message: "Selected files exceed the 102400 byte context limit." });
    await expect(getWorkspaceContext("/tmp/my-app", ["a.txt", "b.txt"])).rejects.toMatchObject({ code: "too_large" });
  });

  it("validates setWorkspace", async () => {
    const inv = {
      rootName: "proj",
      rootPath: "/tmp/proj",
      entries: [],
      truncated: false,
      stats: { fileCount: 0, dirCount: 0, skippedExcluded: 0, skippedSensitive: 0, skippedOversized: 0, skippedUnreadable: 0, skippedSymlinkOutside: 0, skippedDepth: 0 },
      limits: { maxDepth: 12, maxFiles: 8000, maxPreviewBytes: 262144, maxInventoryBytes: 2097152 },
    };
    vi.mocked(invoke).mockResolvedValue(inv);
    const result = await setWorkspace("/tmp/proj");
    expect(result.rootPath).toBe("/tmp/proj");
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("set_workspace", { path: "/tmp/proj" });
  });
});

describe("workspace lib browser fallback", () => {
  beforeEach(() => {
    markDesktop(false);
    vi.mocked(invoke).mockReset();
    clearStoredWorkspace();
  });

  it("throws desktop_only in browser", async () => {
    await expect(scanWorkspace("/tmp/proj")).rejects.toMatchObject({ code: "desktop_only" });
    await expect(readWorkspaceFile("/tmp/proj", "a.txt")).rejects.toMatchObject({ code: "desktop_only" });
    await expect(getWorkspaceContext("/tmp/proj", ["a.txt"])).rejects.toMatchObject({ code: "desktop_only" });
  });
});

describe("workspace storage persistence", () => {
  beforeEach(() => clearStoredWorkspace());
  it("stores and clears workspace path", async () => {
    const { storeWorkspace, loadStoredWorkspace, clearStoredWorkspace: clear } = await import("./workspace");
    storeWorkspace("/tmp/proj");
    expect(loadStoredWorkspace()).toBe("/tmp/proj");
    clear();
    expect(loadStoredWorkspace()).toBeNull();
  });
});
