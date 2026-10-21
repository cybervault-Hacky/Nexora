import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/**
 * Runtime tests for the export IPC wrapper.
 *
 * The Tauri module is replaced by a double, so these prove the wrapper's call
 * shape, channel wiring and error normalisation. They do not write a file and
 * are not evidence that the Rust commands compiled.
 */

const { invoke, Channel, channels } = vi.hoisted(() => {
  const spy = vi.fn();
  const created: Array<{ onmessage: ((event: unknown) => void) | null }> = [];

  class TestChannel<T> {
    onmessage: ((event: T) => void) | null = null;

    constructor() {
      created.push(this as unknown as { onmessage: ((event: unknown) => void) | null });
    }
  }

  return { invoke: spy, Channel: TestChannel, channels: created };
});

vi.mock("@tauri-apps/api/core", () => ({ invoke, Channel }));

import {
  cancelExport,
  exportProjectZip,
  exportSingleFile,
  planWorkspaceExport,
  suggestExportPath,
  toExportError,
  type ExportOutcome,
} from "./export";

const OUTCOME: ExportOutcome = {
  path: "/home/user/project.zip",
  files: 3,
  bytes: 1024,
  cancelled: false,
  replaced: false,
};

function enterDesktopShell() {
  Reflect.set(window, "__TAURI_INTERNALS__", {});
}

beforeEach(() => {
  invoke.mockReset();
  channels.length = 0;
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

describe("error normalisation", () => {
  it("keeps a typed core error intact", () => {
    const failure = { code: "already_exists", message: "It exists." };
    expect(toExportError(failure)).toEqual(failure);
  });

  it("wraps a string and an Error", () => {
    expect(toExportError("no such command")).toEqual({
      code: "ipc",
      message: "no such command",
    });
    expect(toExportError(new Error("channel closed"))).toEqual({
      code: "ipc",
      message: "channel closed",
    });
  });

  it("falls back to a generic message", () => {
    expect(toExportError({ odd: true }).code).toBe("ipc");
  });
});

describe("browser preview", () => {
  it("refuses to plan or export without the desktop shell", async () => {
    await expect(planWorkspaceExport(null)).rejects.toMatchObject({ code: "desktop_only" });
    await expect(suggestExportPath("zip", null)).rejects.toMatchObject({
      code: "desktop_only",
    });
    await expect(
      exportSingleFile(
        { relativePath: "a.txt", destination: "/tmp/a.txt", overwrite: false },
        () => {},
      ),
    ).rejects.toMatchObject({ code: "desktop_only" });
    await expect(
      exportProjectZip({ selection: null, destination: "/tmp/a.zip", overwrite: false }, () => {}),
    ).rejects.toMatchObject({ code: "desktop_only" });
    await expect(cancelExport()).resolves.toBe(false);

    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("planning", () => {
  const plan = {
    rootName: "project",
    files: [{ path: "src/main.rs", size: 30 }],
    excludedSensitive: 1,
    excludedSymlink: 0,
    excludedUnsafe: 0,
    totalBytes: 30,
    truncated: false,
  };

  it("passes null for the whole project", async () => {
    enterDesktopShell();
    invoke.mockResolvedValue(plan);

    await expect(planWorkspaceExport(null)).resolves.toEqual(plan);
    expect(invoke).toHaveBeenCalledWith("plan_workspace_export", { selection: null });
  });

  it("passes an explicit selection", async () => {
    enterDesktopShell();
    invoke.mockResolvedValue(plan);

    await planWorkspaceExport(["src/main.rs"]);
    expect(invoke).toHaveBeenCalledWith("plan_workspace_export", {
      selection: ["src/main.rs"],
    });
  });

  it("asks for a suggested destination with the right kind", async () => {
    enterDesktopShell();
    invoke.mockResolvedValue("/home/user/project.zip");

    await expect(suggestExportPath("zip", null)).resolves.toBe("/home/user/project.zip");
    expect(invoke).toHaveBeenCalledWith("suggest_export_path", {
      kind: "zip",
      relativePath: null,
    });

    await suggestExportPath("file", "src/main.rs");
    expect(invoke).toHaveBeenLastCalledWith("suggest_export_path", {
      kind: "file",
      relativePath: "src/main.rs",
    });
  });

  it("normalises a rejected plan", async () => {
    enterDesktopShell();
    invoke.mockRejectedValue({ code: "no_files", message: "Nothing to export." });

    await expect(planWorkspaceExport(null)).rejects.toMatchObject({ code: "no_files" });
  });
});

describe("exporting", () => {
  it("sends the single-file request and forwards channel events", async () => {
    enterDesktopShell();

    invoke.mockImplementation(async (command: string) => {
      expect(command).toBe("export_single_file");
      channels[0]!.onmessage?.({ type: "progress", done: 0, total: 1, bytesDone: 4, bytesTotal: 8 });
      return OUTCOME;
    });

    const seen: unknown[] = [];
    const result = await exportSingleFile(
      { relativePath: "src/main.rs", destination: "/home/user/main.rs", overwrite: false },
      (event) => seen.push(event),
    );

    expect(result).toEqual(OUTCOME);
    expect(invoke).toHaveBeenCalledWith("export_single_file", {
      request: {
        relativePath: "src/main.rs",
        destination: "/home/user/main.rs",
        overwrite: false,
      },
      onEvent: expect.anything(),
    });
    expect(seen).toEqual([
      { type: "progress", done: 0, total: 1, bytesDone: 4, bytesTotal: 8 },
    ]);
  });

  it("sends the zip request with a selection", async () => {
    enterDesktopShell();
    invoke.mockResolvedValue({ ...OUTCOME, files: 2 });

    await exportProjectZip(
      { selection: ["a.txt", "b.txt"], destination: "/home/user/p.zip", overwrite: true },
      () => {},
    );

    expect(invoke).toHaveBeenCalledWith("export_zip", {
      request: { selection: ["a.txt", "b.txt"], destination: "/home/user/p.zip", overwrite: true },
      onEvent: expect.anything(),
    });
  });

  it("surfaces a typed conflict instead of pretending success", async () => {
    enterDesktopShell();
    invoke.mockRejectedValue({ code: "already_exists", message: "p.zip already exists." });

    await expect(
      exportProjectZip({ selection: null, destination: "/home/user/p.zip", overwrite: false }, () => {}),
    ).rejects.toMatchObject({ code: "already_exists" });
  });

  it("reports a cancelled outcome from the core", async () => {
    enterDesktopShell();
    invoke.mockResolvedValue({ ...OUTCOME, path: "", files: 0, bytes: 0, cancelled: true });

    const result = await exportProjectZip(
      { selection: null, destination: "/home/user/p.zip", overwrite: false },
      () => {},
    );

    expect(result.cancelled).toBe(true);
    expect(result.path).toBe("");
  });
});

describe("cancellation", () => {
  it("asks the core to stop the running export", async () => {
    enterDesktopShell();
    invoke.mockResolvedValue(true);

    await expect(cancelExport()).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith("cancel_export");
  });

  it("reports false when the command fails", async () => {
    enterDesktopShell();
    invoke.mockRejectedValue(new Error("closed"));

    await expect(cancelExport()).resolves.toBe(false);
  });
});
