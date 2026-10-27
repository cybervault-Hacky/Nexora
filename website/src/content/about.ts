/**
 * About page content.
 *
 * Describes the project as it exists in the repository. It deliberately carries
 * no founding story, date, team biographies, investors, customers, awards, or
 * testimonials — none of that has been established for publication, and
 * inventing it would be indistinguishable from lying about the project.
 *
 * Sources consulted:
 * - `apps/desktop/src-tauri/tauri.conf.json` — Windows desktop packaging target.
 * - `apps/desktop/src-tauri/src/inference/{process,settings}.rs` — loopback
 *   inference, user-supplied engine path, pinned engine build reporting.
 * - `apps/desktop/src-tauri/src/inference/acquisition.rs` — verified, user-
 *   initiated model acquisition.
 * - `apps/desktop/src-tauri/src/workspace/*` — local scanning, patches, analysis.
 * - `docs/PHASE_18`–`PHASE_21`, `docs/PUBLIC_BETA_PREPARATION_WINDOWS.md` —
 *   source-level completion versus the absence of any Windows runtime evidence.
 * - `website/src/content/trust.ts` — the privacy and security position this page
 *   must not contradict.
 */

import type { TrustFact } from "./trust";

/** The three parts of the product, stated without implying they are verified. */
export const ABOUT_PIECES: readonly TrustFact[] = [
  {
    term: "The application",
    detail:
      "A Windows desktop shell — a window, a Rust core, and local settings and history files — that handles the interface, files, and downloads. The packaging target is Windows x64; no Windows build has been produced.",
  },
  {
    term: "The model",
    detail:
      "A quantized model file of several gigabytes, downloaded once at a pinned revision and checked against a recorded size and SHA-256 digest before it is installed. Two profiles are registered; neither is bundled.",
  },
  {
    term: "The engine",
    detail:
      "A separate inference server process that loads the model and answers requests on 127.0.0.1. Nexora does not ship it: you point the application at a compatible engine you obtained yourself, and it reports a version mismatch rather than hiding one.",
  },
];

/** Design intentions the project actually follows in its source. */
export const ABOUT_PRINCIPLES: readonly TrustFact[] = [
  {
    term: "Local by default",
    detail:
      "Generation runs against a process on your own machine. The exceptions are explicit and few: a model download you start, an account service if one is configured, and the installer's first-run WebView2 setup.",
  },
  {
    term: "Your machine, your paths",
    detail:
      "You choose where the engine and the model live. The application validates those paths and tells you when something does not match, instead of quietly substituting its own.",
  },
  {
    term: "Claims carry their evidence",
    detail:
      "Capabilities on this site are labelled by how far the evidence goes. A page that describes something as working before it has been run would make every other statement on the site worth less.",
  },
  {
    term: "Nothing is published before it is true",
    detail:
      "No version, checksum, release date, or download link appears anywhere on this site until the artifact behind it exists and has been checked.",
  },
];

/** How far the implementation actually goes today. */
export const ABOUT_STATUS: readonly TrustFact[] = [
  {
    term: "Present in source",
    detail:
      "The application surface, the loopback inference path, verified model acquisition, workspace scanning and patch proposals, local conversation history, and the account and usage scaffolding.",
    status: "verified",
  },
  {
    term: "Not yet verified",
    detail:
      "Anything that requires a Windows build: install, launch, upgrade, engine start, model load, and generation. No Windows binary exists, and no model has ever been loaded.",
    status: "unverified",
  },
  {
    term: "Not demonstrated",
    detail:
      "Code signing, update delivery, quotas or billing, and a public support channel. Each is either unconfigured or unimplemented, and none is offered as a feature.",
    status: "planned",
  },
];

/** Promises this project is not making. */
export const ABOUT_NOT_CLAIMED: readonly string[] = [
  "That Nexora will plan and complete a coding task on its own. It is an assistant in a window, not an autonomous engineer.",
  "That every feature works offline. Model downloads, sign-in, and first-run setup all need a connection.",
  "That the application is production-secure. No audit, penetration test, or certification exists, and the source review describes limits in the same detail as controls.",
  "That a release is ready. No installer, approved version, checksum, signing identity, or Windows test result exists.",
  "That model output is accurate, safe, or suitable for a particular purpose. No benchmark or quality evaluation is published.",
];
