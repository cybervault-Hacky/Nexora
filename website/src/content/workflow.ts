/**
 * Workflow content for the How It Works page.
 *
 * Every statement below is traceable to repository source. Where a figure is
 * quoted, it comes from the authoritative metadata in
 * `apps/desktop/src-tauri/src/inference/registry.rs`; nothing is estimated,
 * rounded from memory, or copied from a blog post.
 *
 * Sources:
 * - `inference/registry.rs` — profile IDs, upstream models, pinned byte sizes
 *   and SHA-256 digests; both profiles carry `locally_verified: false`.
 * - `inference/settings.rs` — `runtime_path: Option<String>`; the engine
 *   executable is user-supplied, not bundled.
 * - `inference/process.rs` — `build_launch_args()` passes `--host 127.0.0.1`,
 *   `--ctx-size`, `--parallel 1`, `--api-key-file`; the session secret is never
 *   placed in process argv.
 * - `inference/readiness.rs` — estimates are labelled as estimates, and an
 *   unreportable memory/disk figure is `unknown`, not "fine".
 * - `inference/acquisition.rs` — a user-initiated download staged in a `.part`
 *   file, verified against SHA-256, then renamed into place.
 * - `workspace/patch.rs`, `analysis.rs`, `symbol_index.rs` — reviewable patches,
 *   dependency graph, symbol index.
 * - `docs/PHASE_18_INTEGRATION_VERIFICATION.md` — typecheck, 88 Vitest files /
 *   870 tests and the production build passed; native Rust, Tauri launch, real
 *   IPC, local inference, OAuth, payments and PostgreSQL were **not** run.
 * - `docs/PHASE_19_WINDOWS_PACKAGING.md` — no Windows binary or installer was
 *   produced, installed, launched, or signed.
 */

import type { CapabilityStatus } from "./capabilities";

export interface WorkflowStep {
  /** Stable anchor id for the step. */
  id: string;
  /** Step title. */
  title: string;
  /** How far the evidence for this step goes. */
  status: CapabilityStatus;
  /** What the step involves, in plain language. */
  body: readonly string[];
  /** One line separating what is designed from what has been demonstrated. */
  evidence: string;
}

export const WORKFLOW_STEPS: readonly WorkflowStep[] = [
  {
    id: "get-the-app",
    title: "Get the Windows application",
    status: "planned",
    body: [
      "Nexora is a native Windows desktop application, not a website or a browser extension. A per-user installer is configured for 64-bit Windows in the project's packaging settings.",
      "No installer has been built, signed, installed, or tested, so there is nothing to download yet. When one is published it will appear on the download page with its version, file size, and checksum.",
    ],
    evidence:
      "Configured in source. Not built, and not verified on Windows.",
  },
  {
    id: "download-the-model",
    title: "Download the required model",
    status: "implemented",
    body: [
      "Model weights are large files published separately from the application, so this step needs an internet connection. Downloads start only when you ask for them.",
      "Nexora writes the transfer into a temporary file, verifies the finished file against a pinned SHA-256 digest, and only then moves it into place. A file that does not match is not used.",
      "The two configured profiles are Nexora Mini at 4.36 GiB (4,683,073,536 bytes) and Nexora Sonnet at 8.37 GiB (8,988,110,272 bytes). Both are Q4_K_M GGUF files of Apache-2.0 licensed Qwen2.5-Coder models, and both are intended to run on the CPU.",
    ],
    evidence:
      "Implemented in source, including digests. No download has been executed here.",
  },
  {
    id: "prepare-and-select",
    title: "Prepare the engine and select a profile",
    status: "implemented",
    body: [
      "Generation runs through a separate local engine executable that Nexora launches. Nexora stores the path to that engine; it does not bundle one, so this is a setup step rather than something that happens invisibly.",
      "You then choose a profile. Before offering it, Nexora estimates whether the machine can hold it — comparing the model file size against reported free disk space, and the working memory the model needs at the configured context length against reported physical memory.",
      "Those figures are arithmetic estimates, not measurements, and Nexora does not publish a minimum hardware specification. When a platform cannot report memory or free disk, the check is reported as unknown rather than assumed to pass.",
    ],
    evidence:
      "Implemented in source. Neither profile has ever been loaded, and both remain flagged as not locally verified.",
  },
  {
    id: "use-the-assistant",
    title: "Work with the coding assistant",
    status: "unverified",
    body: [
      "Once an engine and a profile are in place, generation requests go to a runtime Nexora manages on 127.0.0.1. Responses stream back as they are produced and can be cancelled mid-flight.",
      "Code changes are proposed as patches that are validated against the current file contents before they are applied, so a stale edit is rejected rather than written. The workspace can also index symbols across your files and map how files depend on one another.",
      "All of this exists in the source and is covered by the project's automated UI tests, but those tests use simulated responses. No real model has produced a response in a verified environment.",
    ],
    evidence:
      "Implemented in source with simulated-response tests. Not verified end to end.",
  },
  {
    id: "stay-transparent",
    title: "Keep the boundary visible",
    status: "implemented",
    body: [
      "Core generation is designed to stay on your machine. Optional integrations are a separate concern and are deliberately not described as offline-capable, because they have not been verified that way.",
      "Nexora has not had an external security audit, and this site makes no guarantee about privacy or security. What can be said is architectural: for core generation, requests go to a loopback address rather than to an external AI provider.",
    ],
    evidence:
      "Architecture is present in source. No audit, certification, or guarantee exists.",
  },
];

/** The readiness statement shown near the top of the page. */
export const READINESS_NOTE = {
  title: "What is not yet verified",
  items: [
    "No Windows build or installer has been produced, installed, or launched.",
    "No model has been downloaded, loaded, or used to generate a response.",
    "Local inference has not been verified end to end.",
    "The page below therefore describes the intended workflow and the code behind it — not a demonstrated one.",
  ],
} as const;
