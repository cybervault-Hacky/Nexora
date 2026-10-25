import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../lib/inference", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/inference")>();

  return {
    ...actual,
    fetchSystemInfo: vi.fn(),
    fetchModelProfile: vi.fn(),
    fetchModelReadiness: vi.fn(),
    probeRuntime: vi.fn(),
    validateModel: vi.fn(),
    saveSettings: vi.fn(),
    loadModel: vi.fn(),
    startModelDownload: vi.fn(),
    cancelModelDownload: vi.fn(),
    verifyModel: vi.fn(),
  };
});

import { ModelSetup, progressLabel } from "./ModelSetup";
import {
  cancelModelDownload,
  fetchModelProfile,
  fetchModelReadiness,
  fetchSystemInfo,
  loadModel,
  probeRuntime,
  saveSettings,
  startModelDownload,
  validateModel,
  verifyModel,
} from "../lib/inference";
import { makeReadiness, makeStatus } from "../test/fixtures";
import type { ModelDownloadOutcome, ModelProfile } from "../lib/ipc";

const systemMock = vi.mocked(fetchSystemInfo);
const probeMock = vi.mocked(probeRuntime);
const validateMock = vi.mocked(validateModel);
const saveMock = vi.mocked(saveSettings);
const loadMock = vi.mocked(loadModel);
const profileMock = vi.mocked(fetchModelProfile);
const readinessMock = vi.mocked(fetchModelReadiness);
const downloadMock = vi.mocked(startModelDownload);
const cancelDownloadMock = vi.mocked(cancelModelDownload);
const verifyMock = vi.mocked(verifyModel);

const APPROVED_SHA =
  "509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c";

const notConfigured = makeStatus({ state: "not_configured" });

const miniProfile: ModelProfile = {
  id: "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m",
  displayName: "Nexora Mini",
  publisher: "Qwen team, Alibaba Group",
  upstreamModel: "Qwen/Qwen2.5-Coder-7B-Instruct",
  upstreamRevision: "c03e6d358207e414f1eca0bb1891e29f1db0e242",
  format: "GGUF",
  quantization: "Q4_K_M",
  licence: { id: "Apache-2.0", reference: "https://example.invalid/LICENSE" },
  artifact: {
    state: "approved",
    repository: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF",
    revision: "13fb94bfda8c8cf22497dc57b78f391a9acb426a",
    fileName: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
    sizeBytes: 4_683_073_536,
    sha256: "509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c",
  },
  context: { recommended: 32_768, documentedMax: 32_768 },
  compatibilityNotes: ["note"],
  locallyVerified: false,
};

beforeEach(() => {
  // The setup panel treats the desktop shell as the only place a download can
  // start; jsdom is a browser, so the flag is set explicitly for these tests.
  Reflect.set(window, "__TAURI_INTERNALS__", {});
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
  profileMock.mockReset();
  profileMock.mockResolvedValue(miniProfile);
  readinessMock.mockReset();
  readinessMock.mockResolvedValue(makeReadiness());
  downloadMock.mockReset();
  cancelDownloadMock.mockReset();
  cancelDownloadMock.mockResolvedValue(false);
  verifyMock.mockReset();
});

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

function setup(onStatus = vi.fn(), onModelsChanged = vi.fn()) {
  render(
    <ModelSetup
      status={notConfigured}
      onStatus={onStatus}
      onModelsChanged={onModelsChanged}
    />,
  );
  return { user: userEvent.setup(), onStatus, onModelsChanged };
}

describe("ModelSetup", () => {
  it("explains that downloads are explicit and verified", () => {
    setup();

    expect(
      screen.getByText(/Downloads are explicit and verified/i),
    ).toBeTruthy();
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

  it("renders the core's readiness verbatim and labels the numbers as estimates", async () => {
    setup();

    expect(await screen.findByTestId("model-readiness")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Ready to download" })).toBeTruthy();
    expect(
      screen.getByText(/Nothing is installed yet\. The approved file can be downloaded/i),
    ).toBeTruthy();
    expect(screen.getAllByText(/8\.37 GiB/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/8\.87 GiB/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/≈10\.9 GiB/).length).toBeGreaterThan(0);
    expect(screen.getByText(/are estimates from the published file size/i)).toBeTruthy();
  });

  it("shows the core's unknown answers instead of inventing a number", async () => {
    readinessMock.mockResolvedValue(
      makeReadiness({
        state: "detection_unavailable",
        headline: "System resources unknown",
        detail:
          "Nexora could not read memory or free disk space on this system. The download can start, but neither the fit nor the space is confirmed.",
        checks: [
          {
            id: "storage",
            status: "unknown",
            message: "Free space at the download folder could not be read on this system.",
          },
          {
            id: "memory",
            status: "unknown",
            message: "Memory could not be read on this system.",
          },
        ],
      }),
    );

    setup();

    expect(await screen.findByRole("heading", { name: "System resources unknown" })).toBeTruthy();
    expect(screen.getByText(/could not read memory or free disk space/i)).toBeTruthy();
    expect(screen.getAllByText("Unknown")).toHaveLength(2);
  });

  it("refuses to start a download the core says is blocked, and says why", async () => {
    readinessMock.mockResolvedValue(
      makeReadiness({
        state: "insufficient_storage",
        headline: "Not enough storage",
        detail: "The destination needs 8.87 GiB free; 4.00 GiB is available.",
        canDownload: false,
      }),
    );

    setup();

    const button = await screen.findByRole("button", { name: /Download Nexora Mini/i });

    await waitFor(() => {
      expect((button as HTMLButtonElement).disabled).toBe(true);
    });
    expect(button.getAttribute("title")).toMatch(/needs 8\.87 GiB free/);
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
      architecture: "qwen2",
      name: null,
      contextLength: 32768,
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
      modelId: miniProfile.id,
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
      modelId: miniProfile.id,
    });
    expect(onStatus).toHaveBeenCalledWith(makeStatus({ state: "ready" }));
  });

  it("verifies once and retries when the core refuses an unverified file", async () => {
    saveMock.mockResolvedValue({
      modelPath: "D:\\m\\model.gguf",
      runtimePath: "C:\\llama-server.exe",
      contextSize: 32768,
      modelId: miniProfile.id,
    });
    loadMock
      .mockRejectedValueOnce({
        code: "model_unverified",
        message: "Nexora Mini has not been verified in this session.",
      })
      .mockResolvedValueOnce(makeStatus({ state: "ready" }));
    verifyMock.mockResolvedValue({
      path: "D:\\m\\model.gguf",
      fileName: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
      profileId: miniProfile.id,
      sizeBytes: 4_683_073_536,
      sizeMatches: true,
      sha256: APPROVED_SHA,
      expectedSizeBytes: 4_683_073_536,
      expectedSha256: APPROVED_SHA,
      digestMatches: true,
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

    await waitFor(() => {
      expect(loadMock).toHaveBeenCalledTimes(2);
    });

    expect(verifyMock).toHaveBeenCalledWith("D:\\m\\model.gguf", miniProfile.id);
    expect(onStatus).toHaveBeenCalledWith(makeStatus({ state: "ready" }));
  });

  it("does not load when the retried verification does not match", async () => {
    saveMock.mockResolvedValue({
      modelPath: "D:\\m\\model.gguf",
      runtimePath: "C:\\llama-server.exe",
      contextSize: 32768,
      modelId: miniProfile.id,
    });
    loadMock.mockRejectedValueOnce({
      code: "model_unverified",
      message: "Nexora Mini has not been verified in this session.",
    });
    verifyMock.mockResolvedValue({
      path: "D:\\m\\model.gguf",
      fileName: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
      profileId: miniProfile.id,
      sizeBytes: 4_683_073_536,
      sizeMatches: true,
      sha256: "00".repeat(32),
      expectedSizeBytes: 4_683_073_536,
      expectedSha256: APPROVED_SHA,
      digestMatches: false,
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
    expect(alert.textContent).toMatch(/SHA-256 differs from the approved file/i);

    await waitFor(() => {
      expect(loadMock).toHaveBeenCalledTimes(1);
    });
    expect(verifyMock).toHaveBeenCalledTimes(1);
    expect(onStatus).not.toHaveBeenCalled();
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

  it("refreshes the engine status when a load fails after saving", async () => {
    saveMock.mockResolvedValue({
      modelPath: "D:\\m\\model.gguf",
      runtimePath: "C:\\llama-server.exe",
      contextSize: 32768,
      modelId: miniProfile.id,
    });
    loadMock.mockRejectedValue({
      code: "load_failed",
      message:
        "The local engine could not load the model. The local engine stopped unexpectedly (exit status: 1).",
    });

    const onRefresh = vi.fn();
    const onStatus = vi.fn();
    render(
      <ModelSetup
        status={notConfigured}
        onStatus={onStatus}
        onRefresh={onRefresh}
      />,
    );
    const user = userEvent.setup();

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
    expect(alert.textContent).toContain("could not load the model");
    expect(onRefresh).toHaveBeenCalled();
    // A failed load must never be reported as a ready engine.
    expect(onStatus).not.toHaveBeenCalled();
  });

  it("shows the approved profile before anything is downloaded", async () => {
    setup();

    expect(
      await screen.findByRole("heading", { name: "Nexora Mini" }),
    ).toBeTruthy();
    expect(screen.getByText(/Q4_K_M · Apache-2\.0/)).toBeTruthy();
    expect(screen.getByText(/recommended context 32,768 tokens/)).toBeTruthy();
  });

  it("downloads the approved file and only prefills the path", async () => {
    downloadMock.mockResolvedValue({
      path: "C:\\Models\\qwen2.5-coder-7b-instruct-q4_k_m.gguf",
      fileName: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
      sizeBytes: 4_683_073_536,
      sha256: APPROVED_SHA,
      cancelled: false,
    });

    const { user } = setup();

    await user.click(
      await screen.findByRole("button", { name: "Download Nexora Mini" }),
    );

    expect(downloadMock).toHaveBeenCalledTimes(1);
    expect(downloadMock.mock.calls[0]?.[0]).toBe(miniProfile.id);
    expect(downloadMock.mock.calls[0]?.[1]).toEqual(expect.any(Function));

    const input = screen.getByRole("textbox", {
      name: "Model file (.gguf)",
    }) as HTMLInputElement;

    expect(input.value).toBe(
      "C:\\Models\\qwen2.5-coder-7b-instruct-q4_k_m.gguf",
    );
    expect(saveMock).not.toHaveBeenCalled();
    expect(await screen.findByText(/Downloaded and verified/)).toBeTruthy();
  });

  it("asks the app to re-read the catalogue after a download changes a row", async () => {
    downloadMock.mockResolvedValue({
      path: "C:\\Models\\qwen2.5-coder-7b-instruct-q4_k_m.gguf",
      fileName: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
      sizeBytes: 4_683_073_536,
      sha256: APPROVED_SHA,
      cancelled: false,
    });

    const { user, onModelsChanged } = setup();

    await user.click(
      await screen.findByRole("button", { name: "Download Nexora Mini" }),
    );

    await waitFor(() => {
      expect(onModelsChanged).toHaveBeenCalled();
    });
    // The card asks the core again instead of assuming the download's result.
    expect(readinessMock.mock.calls.length).toBeGreaterThan(1);
  });

  it("offers a cancel while a download is still running", async () => {
    downloadMock.mockReturnValue(new Promise<ModelDownloadOutcome>(() => {}));
    cancelDownloadMock.mockResolvedValue(true);

    const { user } = setup();

    await user.click(
      await screen.findByRole("button", { name: "Download Nexora Mini" }),
    );

    expect(screen.getByRole("button", { name: "Downloading…" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Cancel download" }));

    expect(cancelDownloadMock).toHaveBeenCalledTimes(1);
  });

  it("reports a mismatch instead of claiming the file is approved", async () => {
    verifyMock.mockResolvedValue({
      path: "D:\\m\\other.gguf",
      fileName: "other.gguf",
      profileId: miniProfile.id,
      sizeBytes: 1024,
      sizeMatches: false,
      sha256: "ab",
      expectedSizeBytes: 4_683_073_536,
      expectedSha256: APPROVED_SHA,
      digestMatches: false,
    });

    const { user } = setup();

    await user.type(
      screen.getByRole("textbox", { name: "Model file (.gguf)" }),
      "D:\\m\\other.gguf",
    );
    await user.click(screen.getByRole("button", { name: "Verify my file" }));

    expect(verifyMock).toHaveBeenCalledWith("D:\\m\\other.gguf", miniProfile.id);
    expect(
      await screen.findByText(/It may still be a valid GGUF/),
    ).toBeTruthy();
  });

  it("confirms a file that matches the approved digest", async () => {
    verifyMock.mockResolvedValue({
      path: "C:\\Models\\qwen2.5-coder-7b-instruct-q4_k_m.gguf",
      fileName: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
      profileId: miniProfile.id,
      sizeBytes: 4_683_073_536,
      sizeMatches: true,
      sha256: APPROVED_SHA,
      expectedSizeBytes: 4_683_073_536,
      expectedSha256: APPROVED_SHA,
      digestMatches: true,
    });

    const { user } = setup();

    await user.type(
      screen.getByRole("textbox", { name: "Model file (.gguf)" }),
      "C:\\Models\\qwen2.5-coder-7b-instruct-q4_k_m.gguf",
    );
    await user.click(screen.getByRole("button", { name: "Verify my file" }));

    expect(
      await screen.findByText(/matches the approved Nexora Mini SHA-256/),
    ).toBeTruthy();
  });

  it("never renders a fake success state", async () => {
    setup();

    expect(screen.queryByText(/ready/i)).toBeNull();
    expect(screen.queryByText(/connected/i)).toBeNull();
  });

  it("never claims 100% before verification completes and never fabricates percentages on unknown length", () => {
    expect(
      progressLabel({
        received: 4_683_073_536,
        total: 4_683_073_536,
        verifying: false,
      }),
    ).toContain("(99%)");
    expect(
      progressLabel({
        received: 4_683_073_536,
        total: 4_683_073_536,
        verifying: false,
      }),
    ).not.toContain("100%");

    const unknownLength = progressLabel({
      received: 512 * 1024 * 1024,
      total: 0,
      verifying: false,
    });
    expect(unknownLength).toBe("Downloading 0.50 GiB received…");
    expect(unknownLength).not.toContain("%");

    expect(
      progressLabel({
        received: 4_683_073_536,
        total: 4_683_073_536,
        verifying: true,
      }),
    ).toBe("Verifying the SHA-256 before publishing the file…");
  });

  it("surfaces network/storage download errors clearly and allows immediate retry", async () => {
    downloadMock
      .mockRejectedValueOnce({
        code: "network",
        message: "The model host answered 503 Service Unavailable. No file was written.",
      })
      .mockResolvedValueOnce({
        path: "C:\\Models\\qwen2.5-coder-7b-instruct-q4_k_m.gguf",
        fileName: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
        sizeBytes: 4_683_073_536,
        sha256: APPROVED_SHA,
        cancelled: false,
      });

    const { user } = setup();
    await user.click(await screen.findByRole("button", { name: "Download Nexora Mini" }));

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("503 Service Unavailable");

    await user.click(screen.getByRole("button", { name: "Retry download" }));
    expect(await screen.findByText(/Downloaded and verified/)).toBeTruthy();
    expect(downloadMock).toHaveBeenCalledTimes(2);
  });
});
