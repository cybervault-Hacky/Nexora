import type { ModelCatalog, ModelCatalogEntry, ModelProfile } from "../lib/ipc";
import { makeReadiness } from "./fixtures";

export const MINI_PROFILE_ID = "nexora-mini-qwen2.5-coder-7b-instruct-q4-k-m";
export const SONNET_PROFILE_ID = "nexora-sonnet-qwen2.5-coder-14b-instruct-q4-k-m";

/** A selected, loaded catalogue payload for app-level UI tests only. */
export function makeMiniModelCatalog(): ModelCatalog {
  const profile: ModelProfile = {
    id: MINI_PROFILE_ID,
    displayName: "Nexora Mini",
    publisher: "Nexora UI fixture",
    upstreamModel: "Mini test fixture",
    upstreamRevision: "fixture-revision",
    format: "GGUF",
    quantization: "Q4_K_M",
    licence: { id: "Fixture", reference: "https://example.invalid/license" },
    artifact: {
      state: "approved",
      repository: "fixture/mini",
      revision: "fixture-revision",
      fileName: "mini.gguf",
      sizeBytes: 1,
      sha256: "0".repeat(64),
    },
    context: { recommended: 32768, documentedMax: 32768 },
    compatibilityNotes: [],
    locallyVerified: false,
  };
  const detail = "Fixture reports the selected local model as loaded.";
  const readiness = makeReadiness({
    state: "loaded",
    headline: "Loaded",
    detail,
    canDownload: false,
    canVerify: false,
    canLoad: false,
    canGenerate: true,
  });
  const entry: ModelCatalogEntry = {
    profile,
    selected: true,
    active: true,
    installedPath: "C:\\Models\\mini.gguf",
    downloadable: false,
    status: "loaded",
    detail,
    readiness,
  };

  return {
    entries: [entry],
    running: true,
    loadedProfileId: MINI_PROFILE_ID,
    selectionError: null,
  };
}
