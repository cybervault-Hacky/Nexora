import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/inference", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/inference")>();

  return {
    ...actual,
    fetchSystemInfo: vi.fn(),
    probeRuntime: vi.fn(),
    validateModel: vi.fn(),
    saveSettings: vi.fn(),
    loadModel: vi.fn(),
  };
});

import { ModelSetup } from "./ModelSetup";
import {
  fetchSystemInfo,
  loadModel,
  probeRuntime,
  saveSettings,
  validateModel,
} from "../lib/inference";
import { makeStatus } from "../test/fixtures";

const systemMock = vi.mocked(fetchSystemInfo);
const probeMock = vi.mocked(probeRuntime);
const validateMock = vi.mocked(validateModel);
const saveMock = vi.mocked(saveSettings);
const loadMock = vi.mocked(loadModel);

const notConfigured = makeStatus({ state: "not_configured" });

beforeEach(() => {
  systemMock.mockReset();
  systemMock.mockResolvedValue({
    totalRamGib: 32,
    availableRamGib: 21.4,
    platform: "windows",
  });
  probeMock.mockReset();
  validateMock.mockReset();
  saveMock.mockReset();
  loadMock.mockReset();
});

function setup(onStatus = vi.fn()) {
  render(<ModelSetup status={notConfigured} onStatus={onStatus} />);
  return { user: userEvent.setup(), onStatus };
}

describe("ModelSetup", () => {
  it("explains that nothing is downloaded", () => {
    setup();

    expect(screen.getByText(/nothing is downloaded/i)).toBeTruthy();
    expect(
      screen.getByText(/listens on 127\.0\.0\.1 only/),
    ).toBeTruthy();
  });

  it("disables the load button until both paths are present", () => {
    setup();

    expect(
      (screen.getByRole("button", { name: "Save and load model" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("shows the detected memory instead of a guess", async () => {
    setup();

    expect(
      await screen.findByText("21.4 of 32 GiB RAM available"),
    ).toBeTruthy();
  });

  it("reports a runtime failure from the core verbatim", async () => {
    probeMock.mockRejectedValue({
      code: "runtime_missing",
      message: "No file exists at the runtime path.",
    });

    const { user } = setup();

    await user.type(
      screen.getByRole("textbox", { name: "Runtime executable" }),
      "C:\\Tools\\llama\\llama-server.exe",
    );
    await user.click(screen.getAllByRole("button", { name: "Check" })[0]!);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("No file exists at the runtime path.");
  });

  it("shows the runtime version after a successful probe", async () => {
    probeMock.mockResolvedValue({
      path: "C:\\llama-server.exe",
      versionLine: "version: 11714 (build)",
      pinnedBuildMatched: true,
    });

    const { user } = setup();

    await user.type(
      screen.getByRole("textbox", { name: "Runtime executable" }),
      "C:\\llama-server.exe",
    );
    await user.click(screen.getAllByRole("button", { name: "Check" })[0]!);

    expect(await screen.findByText(/^version: 11714/)).toBeTruthy();
  });

  it("warns when the runtime is not the pinned build", async () => {
    probeMock.mockResolvedValue({
      path: "C:\\llama-server.exe",
      versionLine: "version: 9999 (other)",
      pinnedBuildMatched: false,
    });

    const { user } = setup();

    await user.type(
      screen.getByRole("textbox", { name: "Runtime executable" }),
      "C:\\llama-server.exe",
    );
    await user.click(screen.getAllByRole("button", { name: "Check" })[0]!);

    expect(
      await screen.findByText(/does not report the pinned target/),
    ).toBeTruthy();
  });

  it("stays quiet when the runtime matches the pinned build", async () => {
    probeMock.mockResolvedValue({
      path: "C:\\llama-server.exe",
      versionLine: "version: 11714 (build)",
      pinnedBuildMatched: true,
    });

    const { user } = setup();

    await user.type(
      screen.getByRole("textbox", { name: "Runtime executable" }),
      "C:\\llama-server.exe",
    );
    await user.click(screen.getAllByRole("button", { name: "Check" })[0]!);

    await screen.findByText(/^version: 11714/);
    expect(screen.queryByText(/does not report the pinned target/)).toBeNull();
  });

  it("surfaces model warnings such as an oversized file", async () => {
    validateMock.mockResolvedValue({
      path: "D:\\m\\big.gguf",
      fileName: "big.gguf",
      sizeBytes: 18_000_000_000,
      sizeGib: 16.76,
      ggufVersion: 3,
      tensorCount: 402,
      estimatedMinRamGib: 19.3,
      fitsAvailableRam: false,
      warnings: ["This model may not fit in available memory."],
    });

    const { user } = setup();

    await user.type(
      screen.getByRole("textbox", { name: "Model file (.gguf)" }),
      "D:\\m\\big.gguf",
    );
    await user.click(screen.getAllByRole("button", { name: "Check" })[1]!);

    expect(
      await screen.findByText("This model may not fit in available memory."),
    ).toBeTruthy();
    expect(screen.getByText(/16\.76 GiB/)).toBeTruthy();
  });

  it("saves settings and asks the core to load the model", async () => {
    saveMock.mockResolvedValue({
      modelPath: "D:\\m\\model.gguf",
      runtimePath: "C:\\llama-server.exe",
      contextSize: 32768,
    });
    loadMock.mockResolvedValue(makeStatus({ state: "ready" }));

    const { user, onStatus } = setup();

    await user.type(
      screen.getByRole("textbox", { name: "Runtime executable" }),
      "C:\\llama-server.exe",
    );
    await user.type(
      screen.getByRole("textbox", { name: "Model file (.gguf)" }),
      "D:\\m\\model.gguf",
    );
    await user.click(screen.getByRole("button", { name: "Save and load model" }));

    await waitFor(() => {
      expect(loadMock).toHaveBeenCalled();
    });

    expect(saveMock).toHaveBeenCalledWith({
      modelPath: "D:\\m\\model.gguf",
      runtimePath: "C:\\llama-server.exe",
      contextSize: 32768,
    });
    expect(onStatus).toHaveBeenCalledWith(makeStatus({ state: "ready" }));
  });

  it("keeps the setup screen and shows the load error when the engine fails", async () => {
    saveMock.mockRejectedValue({
      code: "spawn_failed",
      message: "The runtime exited before it became healthy.",
    });

    const { user, onStatus } = setup();

    await user.type(
      screen.getByRole("textbox", { name: "Runtime executable" }),
      "C:\\llama-server.exe",
    );
    await user.type(
      screen.getByRole("textbox", { name: "Model file (.gguf)" }),
      "D:\\m\\model.gguf",
    );
    await user.click(screen.getByRole("button", { name: "Save and load model" }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("exited before it became healthy");
    expect(onStatus).not.toHaveBeenCalled();
  });

  it("never renders a fake success state", async () => {
    setup();

    expect(screen.queryByText(/ready/i)).toBeNull();
    expect(screen.queryByText(/connected/i)).toBeNull();
  });
});
