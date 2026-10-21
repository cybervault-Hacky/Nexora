import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/export", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/export")>();

  return {
    ...actual,
    planWorkspaceExport: vi.fn(),
    suggestExportPath: vi.fn(),
    exportSingleFile: vi.fn(),
    exportProjectZip: vi.fn(),
    cancelExport: vi.fn(),
  };
});

import { ExportPanel } from "./ExportPanel";
import {
  cancelExport,
  exportProjectZip,
  exportSingleFile,
  planWorkspaceExport,
  suggestExportPath,
  type ExportOutcome,
  type ExportPlan,
} from "../lib/export";

const planMock = vi.mocked(planWorkspaceExport);
const suggestMock = vi.mocked(suggestExportPath);
const fileMock = vi.mocked(exportSingleFile);
const zipMock = vi.mocked(exportProjectZip);
const cancelMock = vi.mocked(cancelExport);

const HALF_GIB = 512 * 1024 * 1024;

const PLAN: ExportPlan = {
  rootName: "project",
  files: [
    { path: "src/main.rs", size: HALF_GIB },
    { path: "src/lib.rs", size: HALF_GIB },
    { path: "README.md", size: HALF_GIB },
  ],
  excludedSensitive: 2,
  excludedSymlink: 1,
  excludedUnsafe: 0,
  totalBytes: HALF_GIB * 3,
  truncated: false,
};

const ZIP_OUTCOME: ExportOutcome = {
  path: "/home/you/project.zip",
  files: 3,
  bytes: HALF_GIB * 3,
  cancelled: false,
  replaced: false,
};

const NO_SELECTION: ReadonlySet<string> = new Set<string>();

beforeEach(() => {
  Reflect.set(window, "__TAURI_INTERNALS__", {});
  planMock.mockReset();
  suggestMock.mockReset();
  fileMock.mockReset();
  zipMock.mockReset();
  cancelMock.mockReset();

  planMock.mockResolvedValue(PLAN);
  suggestMock.mockResolvedValue("/home/you/project.zip");
  cancelMock.mockResolvedValue(true);
});

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

function renderPanel(options: {
  previewPath?: string | null;
  selectedPaths?: ReadonlySet<string>;
} = {}) {
  render(
    <ExportPanel
      workspacePath="/home/you/project"
      selectedPaths={options.selectedPaths ?? NO_SELECTION}
      previewPath={options.previewPath ?? null}
    />,
  );

  return userEvent.setup();
}

async function waitForDestination() {
  const input = screen.getByLabelText("Save archive to") as HTMLInputElement;

  await waitFor(() => expect(input.value).toBe("/home/you/project.zip"));

  return input;
}

describe("ExportPanel — review", () => {
  it("lists what will be written and what is withheld", async () => {
    renderPanel();

    expect(await screen.findByText(/3 files · 1\.50 GiB will be written\./)).toBeTruthy();
    expect(screen.getByText("src/main.rs")).toBeTruthy();
    expect(screen.getByText("README.md")).toBeTruthy();
    expect(screen.getByText(/Withheld: 2 secret files withheld, 1 symbolic link skipped\./)).toBeTruthy();
    expect(screen.getByText(/Build output, dependencies, caches/i)).toBeTruthy();
  });

  it("shows an honest note when a project has nothing exportable", async () => {
    planMock.mockRejectedValue({
      code: "no_files",
      message: "This project has no exportable files.",
    });

    renderPanel();

    expect(await screen.findByText("This project has no exportable files.")).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: /Export ZIP/ }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });
});

describe("ExportPanel — ZIP export", () => {
  it("writes the reviewed project and reports the real result", async () => {
    zipMock.mockResolvedValue(ZIP_OUTCOME);
    const user = renderPanel();

    await waitForDestination();
    await user.click(screen.getByRole("button", { name: /Export ZIP/ }));

    await waitFor(() => expect(zipMock).toHaveBeenCalledTimes(1));
    expect(zipMock.mock.calls[0]![0]).toEqual({
      selection: null,
      destination: "/home/you/project.zip",
      overwrite: false,
    });

    expect(await screen.findByText(/Saved 3 files \(1\.50 GiB\) to/)).toBeTruthy();
    expect(screen.queryByText(/cancelled/i)).toBeNull();
  });

  it("exports only the selected files when that scope is chosen", async () => {
    zipMock.mockResolvedValue({ ...ZIP_OUTCOME, files: 2 });
    const selected = new Set(["a.ts", "b.ts"]);
    const user = renderPanel({ selectedPaths: selected });

    await waitForDestination();
    await user.click(screen.getByRole("button", { name: "Selected files (2)" }));

    await waitFor(() =>
      expect(planMock).toHaveBeenLastCalledWith(["a.ts", "b.ts"]),
    );

    await user.click(screen.getByRole("button", { name: /Export ZIP/ }));

    await waitFor(() => expect(zipMock).toHaveBeenCalledTimes(1));
    expect(zipMock.mock.calls[0]![0].selection).toEqual(["a.ts", "b.ts"]);
  });

  it("asks for confirmation before replacing an existing archive", async () => {
    zipMock
      .mockRejectedValueOnce({
        code: "already_exists",
        message: "/home/you/project.zip already exists.",
      })
      .mockResolvedValueOnce({ ...ZIP_OUTCOME, replaced: true });

    const user = renderPanel();

    await waitForDestination();
    await user.click(screen.getByRole("button", { name: /Export ZIP/ }));

    expect(await screen.findByText(/already exists/i)).toBeTruthy();
    expect(screen.queryByText(/^Saved /)).toBeNull();

    await user.click(screen.getByRole("button", { name: "Replace existing file" }));

    await waitFor(() => expect(zipMock).toHaveBeenCalledTimes(2));
    expect(zipMock.mock.calls[1]![0].overwrite).toBe(true);
    expect(await screen.findByText(/The previous file was replaced\./)).toBeTruthy();
  });

  it("never reports success when the write fails", async () => {
    zipMock.mockRejectedValue({
      code: "storage",
      message: "The export could not be saved: disk full.",
    });

    const user = renderPanel();

    await waitForDestination();
    await user.click(screen.getByRole("button", { name: /Export ZIP/ }));

    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByText(/disk full/i)).toBeTruthy();
    expect(screen.queryByText(/^Saved /)).toBeNull();
  });

  it("shows cancellation as cancelled, not as success", async () => {
    let resolve: (value: ExportOutcome) => void = () => {};
    zipMock.mockImplementation(
      () =>
        new Promise<ExportOutcome>((settle) => {
          resolve = settle;
        }),
    );

    const user = renderPanel();

    await waitForDestination();
    await user.click(screen.getByRole("button", { name: /Export ZIP/ }));

    await user.click(await screen.findByRole("button", { name: "Cancel export" }));
    expect(cancelMock).toHaveBeenCalled();

    resolve({ ...ZIP_OUTCOME, path: "", files: 0, bytes: 0, cancelled: true });

    expect(await screen.findByText("Export cancelled. Nothing was written.")).toBeTruthy();
    expect(screen.queryByText(/^Saved /)).toBeNull();
  });

  it("does not start a second export while one is running", async () => {
    let resolve: (value: ExportOutcome) => void = () => {};
    zipMock.mockImplementation(
      () =>
        new Promise<ExportOutcome>((settle) => {
          resolve = settle;
        }),
    );

    const user = renderPanel();

    await waitForDestination();
    const button = screen.getByRole("button", { name: /Export ZIP/ });

    await user.click(button);
    await waitFor(() => expect((button as HTMLButtonElement).disabled).toBe(true));

    await user.click(button);
    expect(zipMock).toHaveBeenCalledTimes(1);

    resolve(ZIP_OUTCOME);
    expect(await screen.findByText(/Saved 3 files/)).toBeTruthy();
  });
});

describe("ExportPanel — single file", () => {
  it("is disabled until a file is previewed", async () => {
    renderPanel();

    const single = await screen.findByRole("button", { name: "Single file" });
    expect((single as HTMLButtonElement).disabled).toBe(true);
  });

  it("copies the previewed file through the single-file command", async () => {
    fileMock.mockResolvedValue({
      path: "/home/you/main.rs",
      files: 1,
      bytes: 30,
      cancelled: false,
      replaced: false,
    });

    const user = renderPanel({ previewPath: "src/main.rs" });

    await user.click(screen.getByRole("button", { name: "Single file" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Save file to")).toBeTruthy(),
    );

    await waitFor(() =>
      expect(suggestMock).toHaveBeenLastCalledWith("file", "src/main.rs"),
    );

    await waitFor(() => expect(screen.getByText("src/main.rs")).toBeTruthy());

    await user.click(screen.getByRole("button", { name: /Export file/ }));

    await waitFor(() => expect(fileMock).toHaveBeenCalledTimes(1));
    expect(fileMock.mock.calls[0]![0]).toMatchObject({
      relativePath: "src/main.rs",
      destination: "/home/you/project.zip",
      overwrite: false,
    });
    expect(await screen.findByText(/Saved 1 file/)).toBeTruthy();
  });
});
