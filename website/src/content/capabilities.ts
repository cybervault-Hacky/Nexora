/**
 * Capability content for the Features page.
 *
 * Every entry here is tied to a specific piece of repository evidence, and the
 * `status` records how far that evidence actually goes. The distinction is the
 * whole point of this file: the availability of a feature in source code is not
 * evidence that it works for a user.
 *
 * Evidence sources consulted:
 * - `apps/desktop/src-tauri/src/inference/*` — local runtime, loopback binding,
 *   streaming client, cancellation, acquisition with SHA-256 verification.
 * - `apps/desktop/src-tauri/src/workspace/*` — patch proposals/batches, symbol
 *   index, dependency graph, change impact.
 * - `apps/desktop/src-tauri/src/history/*` — local conversation persistence.
 * - `apps/desktop/ui/src/components/{ModelSetup,ModelSelector}.test.tsx` and
 *   `App.offlineRegression.test.tsx` — mocked UI coverage.
 * - `docs/PHASE_18_INTEGRATION_VERIFICATION.md` — typecheck, 88 Vitest files /
 *   870 tests and the production build passed; native Rust, Tauri launch, real
 *   IPC, local inference, OAuth, payments and PostgreSQL were **not** run.
 * - `docs/PHASE_19_WINDOWS_PACKAGING.md` — no Windows binary or installer was
 *   produced; no Windows launch, install, or inference was executed.
 * - `inference/registry.rs` — both profiles carry `locally_verified: false`.
 *
 * Do not change a status here without new executed evidence. A passing mocked
 * test, a plausible source file, or a roadmap entry is not end-to-end
 * verification.
 */

export type CapabilityStatus = "implemented" | "unverified" | "planned";

export interface Capability {
  /** Capability name, shown as the entry term. */
  name: string;
  /** One or two sentences describing what exists, in user terms. */
  detail: string;
  /** How far the evidence goes. */
  status: CapabilityStatus;
}

export interface CapabilityGroup {
  /** Anchor id, used for the group's heading id and in-page links. */
  id: string;
  /** Group heading (`h2`). */
  title: string;
  /** Short framing sentence for the group. */
  intro: string;
  capabilities: readonly Capability[];
}

export const CAPABILITY_GROUPS: readonly CapabilityGroup[] = [
  {
    id: "local-model-workflow",
    title: "Local model workflow",
    intro:
      "Nexora is designed so that generation runs against a model on your own machine rather than a remote AI provider.",
    capabilities: [
      {
        name: "Loopback-only inference",
        detail:
          "Generation requests are sent to a locally managed runtime bound to 127.0.0.1, not to an external AI service.",
        status: "implemented",
      },
      {
        name: "Streaming responses with cancellation",
        detail:
          "The local client streams a response as it is produced and can cancel a request that is still in flight.",
        status: "implemented",
      },
      {
        name: "Two model profiles",
        detail:
          "Nexora Mini and Nexora Sonnet are defined with pinned file sizes and SHA-256 digests so a downloaded file can be checked.",
        status: "unverified",
      },
      {
        name: "No third-party inference host",
        detail:
          "No external AI provider endpoint is referenced anywhere in the inference module.",
        status: "implemented",
      },
    ],
  },
  {
    id: "coding-assistance",
    title: "Coding assistance",
    intro:
      "The workspace is built around reviewing changes before they land on your files, and understanding how a codebase fits together.",
    capabilities: [
      {
        name: "Reviewable patch proposals",
        detail:
          "A change is proposed, validated against the current file contents, and only then applied — so a stale or conflicting edit is rejected rather than written.",
        status: "implemented",
      },
      {
        name: "Multi-file batch edits",
        detail:
          "Related edits across several files can be prepared as one batch, validated together, and applied as a unit.",
        status: "implemented",
      },
      {
        name: "Code intelligence",
        detail:
          "The workspace can map file dependencies, describe how files relate to one another, and estimate what a change is likely to affect.",
        status: "implemented",
      },
      {
        name: "Symbol index and search",
        detail:
          "Functions, types, and other symbols are indexed from your source files and can be searched across the workspace.",
        status: "implemented",
      },
    ],
  },
  {
    id: "model-management",
    title: "Model selection and management",
    intro:
      "Model files are large, so Nexora treats acquiring and preparing one as a deliberate, checkable step rather than a background download.",
    capabilities: [
      {
        name: "Profile selection",
        detail:
          "You choose which local profile to work with, and the workspace reports which one is selected.",
        status: "implemented",
      },
      {
        name: "Guided model setup",
        detail:
          "A setup flow walks through preparing a model rather than leaving you to place files by hand.",
        status: "implemented",
      },
      {
        name: "Verified downloads",
        detail:
          "A download is staged in a temporary file, checked against its pinned SHA-256 digest, and only then moved into place.",
        status: "implemented",
      },
      {
        name: "Hardware readiness checks",
        detail:
          "Available memory and disk space are checked before a model is offered, because the files involved are several gigabytes.",
        status: "implemented",
      },
    ],
  },
  {
    id: "privacy-control",
    title: "Privacy and control",
    intro:
      "The local-first design is an architectural property of the current source. It is not a certification, and this page presents no guarantee.",
    capabilities: [
      {
        name: "Your code stays local",
        detail:
          "Inference traffic is loopback-only and no external AI provider is contacted for core generation.",
        status: "implemented",
      },
      {
        name: "No account required",
        detail:
          "Local chat and conversation history are independent of signing in; account features are a separate, optional path.",
        status: "implemented",
      },
      {
        name: "Conversation history on disk",
        detail:
          "Conversations are stored as a local file in the application's own data directory on your machine.",
        status: "implemented",
      },
      {
        name: "Not an audited security claim",
        detail:
          "No certification, external audit, or absolute privacy guarantee is asserted anywhere on this site.",
        status: "planned",
      },
    ],
  },
  {
    id: "windows-desktop",
    title: "Windows desktop experience",
    intro:
      "Nexora is a native desktop application for Windows, built on Tauri. No Windows build has been produced or tested yet, so nothing in this group is presented as working software.",
    capabilities: [
      {
        name: "Native desktop shell",
        detail:
          "The application runs in its own window with a native installer rather than in a browser tab.",
        status: "unverified",
      },
      {
        name: "64-bit Windows target",
        detail:
          "The build is configured for x64 Windows. ARM64 and 32-bit packages are not configured.",
        status: "unverified",
      },
      {
        name: "Installer",
        detail:
          "A per-user installer configuration exists, but no installer has been built, signed, installed, or tested.",
        status: "planned",
      },
    ],
  },
];
