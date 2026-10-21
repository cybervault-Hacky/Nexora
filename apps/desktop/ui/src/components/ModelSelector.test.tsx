import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ModelSelector } from "./ModelSelector";
import type { ModelCatalog } from "../features/models/useModelCatalog";
import type { ModelCatalogEntry, ModelProfile } from "../lib/ipc";
import { makeReadiness } from "../test/fixtures";

const MINI_ID = "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m";
const SONNET_ID = "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m";

function profile(overrides: Partial<ModelProfile>): ModelProfile {
  return {
    id: MINI_ID,
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
    ...overrides,
  };
}

const miniProfile = profile({});

const sonnetProfile = profile({
  id: SONNET_ID,
  displayName: "Nexora Sonnet",
  upstreamModel: "Qwen/Qwen2.5-Coder-14B-Instruct",
  upstreamRevision: "aedcc2d42b622764e023cf882b6652e646b95671",
  artifact: {
    state: "approved",
    repository: "Qwen/Qwen2.5-Coder-14B-Instruct-GGUF",
    revision: "d0a692ef765eefbf2fabb130b3cb2e8917e3d225",
    fileName: "qwen2.5-coder-14b-instruct-q4_k_m.gguf",
    sizeBytes: 8_988_110_272,
    sha256: "c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940",
  },
  context: { recommended: 8_192, documentedMax: 32_768 },
});

function entry(overrides: Partial<ModelCatalogEntry>): ModelCatalogEntry {
  return {
    profile: miniProfile,
    selected: false,
    active: false,
    installedPath: null,
    downloadable: true,
    status: "ready_to_download",
    detail: "Nothing is installed yet.",
    readiness: makeReadiness(),
    ...overrides,
  };
}

function catalog(overrides: Partial<ModelCatalog> = {}): ModelCatalog {
  const miniEntry = entry({
    profile: miniProfile,
    selected: true,
    active: true,
    installedPath: "C:\\Models\\qwen2.5-coder-7b-instruct-q4_k_m.gguf",
    status: "loaded",
    detail: "This model is loaded and answering from this machine.",
    readiness: makeReadiness({
      state: "loaded",
      headline: "Loaded",
      detail: "This model is loaded and answering from this machine.",
      canDownload: false,
    }),
  });

  const sonnetEntry = entry({
    profile: sonnetProfile,
    status: "insufficient_memory",
    detail:
      "The model is installed and verified, but this machine does not have enough available memory to load it.",
    readiness: makeReadiness({
      state: "insufficient_memory",
      headline: "Not enough memory",
      detail:
        "The model is installed and verified, but this machine does not have enough available memory to load it. About 10.9 GiB is needed. An estimate, not a hard minimum.",
      canDownload: false,
      checks: [
        {
          id: "memory",
          status: "blocked",
          message: "About 10.9 GiB of memory is needed, but only 3.0 of 3.0 GiB is available.",
        },
      ],
    }),
  });

  return {
    entries: [miniEntry, sonnetEntry],
    selected: miniEntry,
    running: true,
    loadedProfileId: MINI_ID,
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

describe("ModelSelector", () => {
  it("labels each model from the core's status, not from an assumption", () => {
    render(<ModelSelector catalog={catalog()} />);

    const options = screen.getAllByRole("option").map((option) => option.textContent);

    expect(options).toEqual(["Nexora Mini — Loaded", "Nexora Sonnet — Not enough memory"]);
  });

  it("never presents an unapproved model as ready", () => {
    render(<ModelSelector catalog={catalog()} />);

    const options = screen.getAllByRole("option").map((option) => option.textContent ?? "");

    expect(options.some((label) => label.includes("Sonnet") && label.includes("Ready"))).toBe(
      false,
    );
  });

  it("switches the selected profile through the core", async () => {
    const select = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();

    render(<ModelSelector catalog={catalog({ select })} />);

    await user.selectOptions(screen.getByLabelText("Model"), SONNET_ID);

    expect(select).toHaveBeenCalledWith(SONNET_ID);
  });

  it("shows the core's selection error instead of claiming the model is answering", () => {
    const selectedSonnet = entry({
      profile: sonnetProfile,
      selected: true,
      downloadable: true,
      status: "ready_to_download",
      detail: "Nothing is installed yet.",
      readiness: makeReadiness(),
    });

    render(
      <ModelSelector
        catalog={catalog({
          entries: [
            entry({
              profile: miniProfile,
              status: "loaded",
              readiness: makeReadiness({
                state: "loaded",
                headline: "Loaded",
                detail: "This model is loaded and answering from this machine.",
              }),
            }),
            selectedSonnet,
          ],
          selected: selectedSonnet,
          loadedProfileId: MINI_ID,
          selectionError:
            "Nexora Sonnet is selected, but the engine has Nexora Mini loaded. Load the selected model before sending.",
          select: vi.fn(),
        })}
      />,
    );

    expect(
      screen.getByText(/the engine has Nexora Mini loaded/),
    ).toBeTruthy();
    expect(screen.queryByText(/loaded and answering/i)).toBeNull();
  });

  it("shows a refused switch in the core's words and keeps the selection", () => {
    // The core refuses a switch while a reply streams; the selector must not
    // look switched, and the sentence must say why.
    render(
      <ModelSelector
        catalog={catalog({
          switchError:
            "A reply is still streaming. Stop it before switching to Nexora Sonnet.",
          select: vi.fn(),
        })}
      />,
    );

    expect(screen.getByText(/still streaming/)).toBeTruthy();
    // The old selection is still what the core reports as selected.
    expect((screen.getByLabelText("Model") as HTMLSelectElement).value).toBe(MINI_ID);
    expect(screen.queryByText(/loaded and answering/i)).toBeNull();
  });

  it("is disabled while a reply streams", () => {
    render(<ModelSelector catalog={catalog()} disabled />);

    expect((screen.getByLabelText("Model") as HTMLSelectElement).disabled).toBe(true);
  });

  it("shows the core's readiness sentence, not the profile's old detail", () => {
    const base = catalog();

    render(<ModelSelector catalog={catalog({ selected: base.entries[1] })} />);

    expect(
      screen.getByText(/does not have enough available memory to load it/i),
    ).toBeTruthy();
    expect(screen.queryByText(/Nothing is installed yet/i)).toBeNull();
  });

  it("marks the status line with the backend state for styling", () => {
    const base = catalog();

    render(<ModelSelector catalog={catalog({ selected: base.entries[1] })} />);

    expect(screen.getByText(/does not have enough available memory/i).getAttribute("data-state")).toBe(
      "insufficient_memory",
    );
  });

  it("never shows a state the backend did not send", () => {
    render(<ModelSelector catalog={catalog()} />);

    const options = screen.getAllByRole("option").map((option) => option.textContent ?? "");
    const sonnet = options.find((label) => label.includes("Sonnet")) ?? "";

    expect(sonnet).toBe("Nexora Sonnet — Not enough memory");
    expect(sonnet).not.toMatch(/Ready|Loaded/);
  });

  it("renders nothing in the browser preview, where there is no core", () => {
    const { container } = render(
      <ModelSelector
        catalog={catalog({ entries: [], selected: null, loadedProfileId: null })}
      />,
    );

    expect(container.firstChild).toBeNull();
  });
});
