import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class MockChannel {
    onmessage: (msg: unknown) => void = () => {};
  },
}));

vi.mock("./lib/inference", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/inference")>();
  return {
    ...actual,
    generate: vi.fn(),
    cancelGeneration: vi.fn().mockResolvedValue(true),
  };
});

import { invoke } from "@tauri-apps/api/core";
import App from "./App";
import { NEXORA_MINI_ID, NEXORA_SONNET_ID } from "./features/models/readiness";
import { clearFallbackStorage } from "./lib/history";
import { generate } from "./lib/inference";
import type { ModelCatalog, ModelCatalogEntry, ModelProfile } from "./lib/ipc";
import { makeReadiness, makeStatus, makeUsage, markDesktop } from "./test/fixtures";

const invokeMock = vi.mocked(invoke);
const generateMock = vi.mocked(generate);

const MINI_PROFILE: ModelProfile = {
  id: NEXORA_MINI_ID,
  displayName: "Nexora Mini",
  publisher: "Qwen team, Alibaba Group",
  upstreamModel: "Qwen/Qwen2.5-Coder-7B-Instruct",
  upstreamRevision: "c03e6d358207e414f1eca0bb1891e29f1db0e242",
  format: "GGUF",
  quantization: "Q4_K_M",
  licence: {
    id: "Apache-2.0",
    reference: "https://huggingface.co/Qwen/Qwen2.5-Coder-7B-Instruct-GGUF/blob/main/LICENSE",
  },
  artifact: {
    state: "approved",
    repository: "Qwen/Qwen2.5-Coder-7B-Instruct-GGUF",
    revision: "13fb94bfda8c8cf22497dc57b78f391a9acb426a",
    fileName: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
    sizeBytes: 4_683_073_536,
    sha256: "509287f78cb4d4cf6b3843734733b914b2c158e43e22a7f4bf5e963800894d3c",
  },
  context: { recommended: 32_768, documentedMax: 32_768 },
  compatibilityNotes: ["Single-file GGUF"],
  locallyVerified: false,
};

const SONNET_PROFILE: ModelProfile = {
  id: NEXORA_SONNET_ID,
  displayName: "Nexora Sonnet",
  publisher: "Qwen team, Alibaba Group",
  upstreamModel: "Qwen/Qwen2.5-Coder-14B-Instruct",
  upstreamRevision: "aedcc2d42b622764e023cf882b6652e646b95671",
  format: "GGUF",
  quantization: "Q4_K_M",
  licence: {
    id: "Apache-2.0",
    reference:
      "https://huggingface.co/Qwen/Qwen2.5-Coder-14B-Instruct-GGUF/blob/d0a692ef765eefbf2fabb130b3cb2e8917e3d225/LICENSE",
  },
  artifact: {
    state: "approved",
    repository: "Qwen/Qwen2.5-Coder-14B-Instruct-GGUF",
    revision: "d0a692ef765eefbf2fabb130b3cb2e8917e3d225",
    fileName: "qwen2.5-coder-14b-instruct-q4_k_m.gguf",
    sizeBytes: 8_988_110_272,
    sha256: "c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940",
  },
  context: { recommended: 8_192, documentedMax: 32_768 },
  compatibilityNotes: ["Single-file GGUF"],
  locallyVerified: false,
};

function makeCatalog(selectedId: string, loadedId: string): ModelCatalog {
  const miniEntry: ModelCatalogEntry = {
    profile: MINI_PROFILE,
    selected: selectedId === NEXORA_MINI_ID,
    active: loadedId === NEXORA_MINI_ID,
    installedPath: "C:\\Models\\qwen2.5-coder-7b-instruct-q4_k_m.gguf",
    downloadable: true,
    status: loadedId === NEXORA_MINI_ID ? "loaded" : "ready_to_load",
    detail:
      loadedId === NEXORA_MINI_ID
        ? "This model is loaded and answering from this machine."
        : "Installed and verified on disk.",
    readiness: makeReadiness({
      state: loadedId === NEXORA_MINI_ID ? "loaded" : "ready_to_load",
      canLoad: loadedId !== NEXORA_MINI_ID,
      canGenerate: loadedId === NEXORA_MINI_ID,
    }),
  };

  const sonnetEntry: ModelCatalogEntry = {
    profile: SONNET_PROFILE,
    selected: selectedId === NEXORA_SONNET_ID,
    active: loadedId === NEXORA_SONNET_ID,
    installedPath: "C:\\Models\\qwen2.5-coder-14b-instruct-q4_k_m.gguf",
    downloadable: true,
    status: loadedId === NEXORA_SONNET_ID ? "loaded" : "ready_to_load",
    detail:
      loadedId === NEXORA_SONNET_ID
        ? "This model is loaded and answering from this machine."
        : "Installed and verified on disk.",
    readiness: makeReadiness({
      state: loadedId === NEXORA_SONNET_ID ? "loaded" : "ready_to_load",
      canLoad: loadedId !== NEXORA_SONNET_ID,
      canGenerate: loadedId === NEXORA_SONNET_ID,
    }),
  };

  const selectionError =
    selectedId !== loadedId
      ? `${selectedId === NEXORA_SONNET_ID ? "Nexora Sonnet" : "Nexora Mini"} is selected, but the engine has ${loadedId === NEXORA_SONNET_ID ? "Nexora Sonnet" : "Nexora Mini"} loaded. Load the selected model before sending.`
      : null;

  return {
    entries: [miniEntry, sonnetEntry],
    running: true,
    loadedProfileId: loadedId,
    selectionError,
  };
}

describe("Phase 08 — Dedicated Chat Header Model Selector Integration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    clearFallbackStorage();
    window.location.hash = "";
    markDesktop(true);
  });

  afterEach(() => {
    markDesktop(false);
  });

  it("switches from Nexora Mini to Nexora Sonnet inside the shared Chat header while preserving active conversation, unsent draft, and per-turn profileId", async () => {
    let selectedId = NEXORA_MINI_ID;
    let loadedId = NEXORA_MINI_ID;

    invokeMock.mockImplementation((async (cmd: string, rawArgs?: unknown) => {
      const args = (rawArgs ?? {}) as Record<string, unknown>;
      switch (cmd) {
        case "get_app_status":
          return makeStatus({
            state: "ready",
            model:
              loadedId === NEXORA_SONNET_ID
                ? "qwen2.5-coder-14b-instruct-q4_k_m.gguf"
                : "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
            detail: "Ready",
          });
        case "get_model_catalog":
          return makeCatalog(selectedId, loadedId);
        case "select_model":
          selectedId = String(args?.id ?? "");
          return {
            modelPath: "C:\\Models\\model.gguf",
            runtimePath: "C:\\Tools\\llama-server.exe",
            contextSize: 8192,
            modelId: selectedId,
          };
        case "load_model":
          loadedId = selectedId;
          return makeStatus({
            state: "ready",
            model: "qwen2.5-coder-14b-instruct-q4_k_m.gguf",
            detail: "Ready",
          });
        case "get_usage_status":
          return makeUsage();
        case "auth_get_session":
          return { user: null, authenticated: false };
        case "list_conversations":
          return [];
        case "save_conversation":
          return args?.conversation;
        default:
          return null;
      }
    }) as typeof invoke);

    const submittedProfiles: Array<string | null | undefined> = [];
    generateMock.mockImplementation(async (request, onEvent) => {
      submittedProfiles.push(request.profileId);
      const reply =
        request.profileId === NEXORA_SONNET_ID
          ? "Sonnet multi-file analysis complete."
          : "Mini fast helper answer.";
      onEvent({ type: "delta", requestId: request.requestId, text: reply });
      return { requestId: request.requestId, finishReason: "stop", cancelled: false };
    });

    const user = userEvent.setup();
    render(<App />);

    // Selector renders inside `.conversation-header` showing Nexora Mini
    const trigger = await screen.findByTestId("model-selector-trigger");
    expect(trigger.closest(".conversation-header")).toBeTruthy();
    expect(within(trigger).getByText("Nexora Mini")).toBeTruthy();

    // Send first prompt on Nexora Mini
    const composer = (await screen.findByRole("textbox", {
      name: "Message Nexora",
    })) as HTMLTextAreaElement;
    await user.type(composer, "Write a unit test");
    await user.click(screen.getByRole("button", { name: "Send message" }));

    expect(
      await within(screen.getByRole("main")).findByText("Mini fast helper answer."),
    ).toBeTruthy();

    // Type an unsent draft BEFORE switching to Nexora Sonnet
    await user.type(composer, "Refactor across modules carefully");

    // Open header popover and select Nexora Sonnet
    await user.click(trigger);
    const popover = await screen.findByTestId("model-selector-popover");
    await user.click(within(popover).getByTestId(`model-option-${NEXORA_SONNET_ID}`));

    await waitFor(() => expect(within(trigger).getByText("Nexora Sonnet")).toBeTruthy());

    // Transcript AND unsent draft remain untouched on the same Chat page
    expect(composer.value).toBe("Refactor across modules carefully");
    expect(
      within(screen.getByRole("main")).getByText("Mini fast helper answer."),
    ).toBeTruthy();

    // Because Mini is still loaded while Sonnet is selected, sending is blocked cleanly (no silent Mini fallback!)
    const sendButton = screen.getByRole("button", { name: "Send message" }) as HTMLButtonElement;
    expect(sendButton.disabled).toBe(true);

    // Load Sonnet directly from the header selector's quick-load button
    await user.click(await screen.findByTestId("selector-quick-load-btn"));
    await waitFor(() => expect(sendButton.disabled).toBe(false));

    // Wait past submission debounce window and submit second turn under Nexora Sonnet
    await new Promise((r) => setTimeout(r, 320));
    await user.click(sendButton);

    expect(
      await within(screen.getByRole("main")).findByText("Sonnet multi-file analysis complete."),
    ).toBeTruthy();

    // Verify exact per-turn profile IDs sent to core
    expect(submittedProfiles).toEqual([NEXORA_MINI_ID, NEXORA_SONNET_ID]);
  });

  it("blocks unsafe switching during active streaming and keeps submitted request profileId immutable", async () => {
    const selectedId = NEXORA_MINI_ID;

    invokeMock.mockImplementation((async (cmd: string, rawArgs?: unknown) => {
      const args = (rawArgs ?? {}) as Record<string, unknown>;
      if (cmd === "get_app_status") {
        return makeStatus({
          state: "ready",
          model: "qwen2.5-coder-7b-instruct-q4_k_m.gguf",
          detail: "Ready",
        });
      }
      if (cmd === "get_model_catalog") return makeCatalog(selectedId, NEXORA_MINI_ID);
      if (cmd === "get_usage_status") return makeUsage();
      if (cmd === "auth_get_session") return { user: null, authenticated: false };
      if (cmd === "list_conversations") return [];
      if (cmd === "save_conversation") return args?.conversation;
      return null;
    }) as typeof invoke);

    let finishStream!: () => void;
    let capturedRequestProfile: string | null | undefined;
    generateMock.mockImplementation(async (request, onEvent) => {
      capturedRequestProfile = request.profileId;
      onEvent({ type: "delta", requestId: request.requestId, text: "Streaming chunk 1..." });
      await new Promise<void>((resolve) => {
        finishStream = resolve;
      });
      return { requestId: request.requestId, finishReason: "stop", cancelled: false };
    });

    const user = userEvent.setup();
    render(<App />);

    const trigger = await screen.findByTestId("model-selector-trigger");
    const composer = await screen.findByRole("textbox", { name: "Message Nexora" });
    await user.type(composer, "Stream this answer");
    await user.click(screen.getByRole("button", { name: "Send message" }));

    // While streaming, header badge says "Streaming" and Sonnet option is disabled
    await waitFor(() => expect(within(trigger).getByText("Streaming")).toBeTruthy());
    await user.click(trigger);
    const popover = await screen.findByTestId("model-selector-popover");
    expect(
      within(popover).getByText(/stop the current reply before switching models/i),
    ).toBeTruthy();
    const sonnetOption = within(popover).getByTestId(
      `model-option-${NEXORA_SONNET_ID}`,
    ) as HTMLButtonElement;
    expect(sonnetOption.disabled).toBe(true);

    finishStream();
    await waitFor(() => expect(within(trigger).queryByText("Streaming")).toBeNull());
    expect(capturedRequestProfile).toBe(NEXORA_MINI_ID);
  });
});
