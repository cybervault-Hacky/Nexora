import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../lib/inference", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/inference")>();
  return {
    ...actual,
    fetchModelCatalog: vi.fn(),
    selectModel: vi.fn(),
  };
});

import { fetchModelCatalog, selectModel } from "../../lib/inference";
import type { ModelCatalog as Payload, ModelCatalogEntry, ModelProfile } from "../../lib/ipc";
import { useModelCatalog } from "./useModelCatalog";
import { markDesktop, makeReadiness } from "../../test/fixtures";

const MINI_ID = "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m";
const SONNET_ID = "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m";

const fetchMock = vi.mocked(fetchModelCatalog);
const selectMock = vi.mocked(selectModel);

function profile(id: string, displayName: string): ModelProfile {
  return {
    id,
    displayName,
    publisher: "Qwen team, Alibaba Group",
    upstreamModel: "Qwen/Qwen2.5-Coder",
    upstreamRevision: "d0a692ef765eefbf2fabb130b3cb2e8917e3d225",
    format: "GGUF",
    quantization: "Q4_K_M",
    licence: { id: "Apache-2.0", reference: "https://example.invalid/LICENSE" },
    artifact: {
      state: "approved",
      repository: "Qwen/Qwen2.5-Coder-14B-Instruct-GGUF",
      revision: "d0a692ef765eefbf2fabb130b3cb2e8917e3d225",
      fileName: "qwen2.5-coder-14b-instruct-q4_k_m.gguf",
      sizeBytes: 8_988_110_272,
      sha256: "c1e659736d89ac1065fb495330fb824d94001974a4bfa78e7270e43476a8d940",
    },
    context: { recommended: 8_192, documentedMax: 32_768 },
    compatibilityNotes: ["note"],
    locallyVerified: false,
  };
}

function entry(overrides: Partial<ModelCatalogEntry>): ModelCatalogEntry {
  return {
    profile: profile(MINI_ID, "Nexora Mini"),
    selected: false,
    active: false,
    installedPath: null,
    downloadable: true,
    status: "loaded",
    detail: "This model is loaded and answering from this machine.",
    readiness: makeReadiness({ state: "loaded", canGenerate: true }),
    ...overrides,
  };
}

/** A catalog where `selectedId` is the chosen row. */
function catalog(selectedId: string, overrides: Partial<Payload> = {}): Payload {
  const mini = entry({ profile: profile(MINI_ID, "Nexora Mini"), selected: selectedId === MINI_ID });
  const sonnet = entry({
    profile: profile(SONNET_ID, "Nexora Sonnet"),
    selected: selectedId === SONNET_ID,
    status: "ready_to_load",
    detail: "The model is installed and verified and can be loaded.",
    readiness: makeReadiness({ state: "ready_to_load", canLoad: true }),
  });

  return {
    entries: [mini, sonnet],
    running: true,
    loadedProfileId: MINI_ID,
    selectionError: null,
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  markDesktop(true);
  fetchMock.mockReset();
  selectMock.mockReset();
});

describe("useModelCatalog — profile switching", () => {
  it("reads the selected row from the core, not from the profile list order", async () => {
    fetchMock.mockResolvedValue(catalog(SONNET_ID));

    const { result } = renderHook(() => useModelCatalog());

    await waitFor(() => expect(result.current.loading).toBe(false));

    // Sonnet is second in the list; the core's `selected` flag decides.
    expect(result.current.selected?.profile.id).toBe(SONNET_ID);
    expect(result.current.loadedProfileId).toBe(MINI_ID);
  });

  it("marks the selected row only after the core confirms the switch", async () => {
    fetchMock.mockResolvedValueOnce(catalog(MINI_ID)).mockResolvedValueOnce(catalog(SONNET_ID));
    selectMock.mockResolvedValue({
      modelPath: null,
      runtimePath: null,
      contextSize: 8_192,
      modelId: SONNET_ID,
    });

    const { result } = renderHook(() => useModelCatalog());
    await waitFor(() => expect(result.current.selected?.profile.id).toBe(MINI_ID));

    await act(async () => {
      await result.current.select(SONNET_ID);
    });

    expect(selectMock).toHaveBeenCalledWith(SONNET_ID);
    // The hook re-reads rather than assuming: the second fetch is the truth.
    expect(fetchMock).toHaveBeenCalledTimes(2);
    await waitFor(() => expect(result.current.selected?.profile.id).toBe(SONNET_ID));
    expect(result.current.switchError).toBeNull();
  });

  it("keeps the old selection and reports why when the core refuses", async () => {
    fetchMock.mockResolvedValue(catalog(MINI_ID));
    // The core refuses a switch during a generation with a `busy` error.
    selectMock.mockRejectedValue({
      code: "busy",
      message: "A reply is still streaming. Stop it before switching to Nexora Sonnet.",
    });

    const { result } = renderHook(() => useModelCatalog());
    await waitFor(() => expect(result.current.selected?.profile.id).toBe(MINI_ID));

    await act(async () => {
      await result.current.select(SONNET_ID);
    });

    await waitFor(() => expect(result.current.switchError).not.toBeNull());

    // The sentence is the core's, and the selection really did not move.
    expect(result.current.switchError).toContain("still streaming");
    expect(result.current.selected?.profile.id).toBe(MINI_ID);
    expect(result.current.selecting).toBe(false);
  });

  it("clears a refusal once a later switch succeeds", async () => {
    fetchMock
      .mockResolvedValueOnce(catalog(MINI_ID))
      .mockResolvedValueOnce(catalog(MINI_ID))
      .mockResolvedValue(catalog(SONNET_ID));
    selectMock
      .mockRejectedValueOnce({ code: "busy", message: "A model is still loading." })
      .mockResolvedValueOnce({
        modelPath: null,
        runtimePath: null,
        contextSize: 8_192,
        modelId: SONNET_ID,
      });

    const { result } = renderHook(() => useModelCatalog());
    await waitFor(() => expect(result.current.loading).toBe(false));

    await act(async () => {
      await result.current.select(SONNET_ID);
    });
    await waitFor(() => expect(result.current.switchError).toContain("loading"));

    await act(async () => {
      await result.current.select(SONNET_ID);
    });

    await waitFor(() => expect(result.current.switchError).toBeNull());
    await waitFor(() => expect(result.current.selected?.profile.id).toBe(SONNET_ID));
  });

  it("starts Nexora Mini when the core has recorded no choice", async () => {
    // The default profile is the core's decision; the hook must show whatever
    // `selected` came back, which is Mini for an untouched installation.
    fetchMock.mockResolvedValue(catalog(MINI_ID));

    const { result } = renderHook(() => useModelCatalog());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.selected?.profile.displayName).toBe("Nexora Mini");
  });

  it("filters out unapproved profiles and refuses unknown profile IDs without silent fallback", async () => {
    const base = catalog(MINI_ID);
    const rogue = entry({
      profile: profile("unapproved-cloud-model-99b", "Rogue Cloud Model"),
      selected: false,
    });
    fetchMock.mockResolvedValue({ ...base, entries: [...base.entries, rogue] });

    const { result } = renderHook(() => useModelCatalog());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.entries.map((item) => item.profile.id)).toEqual([MINI_ID, SONNET_ID]);

    await act(async () => {
      await result.current.select("unknown-profile-id");
    });

    expect(selectMock).not.toHaveBeenCalled();
    expect(result.current.switchError).toContain("No approved model profile is registered");
    expect(result.current.selected?.profile.id).toBe(MINI_ID);
  });

  it("never substitutes Mini when no approved profile is marked selected", async () => {
    const unselected = catalog("none");
    fetchMock.mockResolvedValue(unselected);

    const { result } = renderHook(() => useModelCatalog());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.selected).toBeNull();
    expect(result.current.selectionError).toContain("unrecognized or unavailable");
  });

  it("discards stale out-of-order fetchModelCatalog responses", async () => {
    let resolveSlowFirst!: (value: Payload) => void;
    const slowFirstPromise = new Promise<Payload>((resolve) => {
      resolveSlowFirst = resolve;
    });

    fetchMock
      .mockImplementationOnce(() => slowFirstPromise)
      .mockResolvedValueOnce(catalog(SONNET_ID));

    const { result, rerender } = renderHook(({ key }) => useModelCatalog(key), {
      initialProps: { key: 0 },
    });

    // Trigger newer refresh (seq 2 -> SONNET_ID) before seq 1 settles
    rerender({ key: 1 });
    await waitFor(() => expect(result.current.selected?.profile.id).toBe(SONNET_ID));

    // Resolve stale seq 1 with MINI_ID afterwards — must be completely ignored
    await act(async () => {
      resolveSlowFirst(catalog(MINI_ID));
    });

    expect(result.current.selected?.profile.id).toBe(SONNET_ID);
  });
});
