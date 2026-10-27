/**
 * Windows release metadata for the download page.
 *
 * ## Why this is `null`
 *
 * No Windows installer exists. Verified on 2026-10-27 against the repository:
 *
 * - No `.exe`, `.msi`, `.msix`, `SHA256SUMS*`, or release manifest exists
 *   anywhere in the working tree, and no `.exe`/`.msi` appears anywhere in Git
 *   history.
 * - `gh release list` returns **no releases** — nothing has been published.
 * - `docs/PHASE_19_WINDOWS_PACKAGING.md`: "no Windows installer was produced".
 * - `docs/PHASE_20_PUBLISHING_PREPARATION.md`: "No local or remote Git tags,
 *   GitHub releases, changelog, or release-notes file were found."
 * - `docs/PHASE_21_FINAL_APPROVAL_AND_RELEASE.md`: "NO-GO — RELEASE BLOCKED",
 *   and repository version `0.1.0` is explicitly *not* an approved release
 *   version.
 *
 * ## Rules for filling this in later
 *
 * Do not populate this object from build configuration, a Tauri/NSIS target, a
 * package.json version, or a CI job definition. Those describe intent, not an
 * artifact. Populate it only from a real, published installer, and only with
 * values measured from that exact file:
 *
 * - `version` — the version approved for release, not the manifest default.
 * - `architecture` — the architecture actually built (no ARM64 claim unless built).
 * - `fileName` — the exact published filename.
 * - `sizeBytes` — measured from the artifact.
 * - `sha256` — computed from that artifact and independently re-checked.
 * - `releasedAt` — the actual publication date.
 * - `releaseNotesUrl` — only if the notes exist and the URL resolves.
 *
 * Leave the whole value `null` until every field is verified. A partially
 * filled object is a worse outcome than an honest "not available yet".
 */

export interface ReleaseMetadata {
  /** Approved release version, e.g. "1.0.0". */
  version: string;
  /** Architecture actually built and tested, e.g. "x64". */
  architecture: string;
  /** Exact published installer filename. */
  fileName: string;
  /** Measured size of the published artifact, in bytes. */
  sizeBytes: number;
  /** SHA-256 hex digest computed from the published artifact. */
  sha256: string;
  /** Publication date, ISO 8601 (`YYYY-MM-DD`). */
  releasedAt: string;
  /** Release notes, only when they exist as a real page. */
  releaseNotesUrl?: string;
}

/**
 * The current published release, or `null` when nothing has been published.
 *
 * This is `null` today. See the module docs above before changing it.
 */
export const CURRENT_RELEASE: ReleaseMetadata | null = null;

/**
 * The checks a release must pass before this page will link to it.
 *
 * These are drawn from the outstanding blockers recorded across
 * `docs/PHASE_19`–`PHASE_21` and `docs/PUBLIC_BETA_PREPARATION_WINDOWS.md`,
 * restated in user-facing terms. They are commitments, not completed work.
 */
export const RELEASE_PREREQUISITES: readonly {
  title: string;
  detail: string;
}[] = [
  {
    title: "A real Windows build",
    detail:
      "The application has to be compiled for x64 Windows on a genuine Windows machine. It has not been: the packaging configuration exists, but no build has been produced.",
  },
  {
    title: "A verified install and launch",
    detail:
      "Installing, launching, upgrading, and uninstalling have to be tested on real Windows 10/11 hardware. None of that has run.",
  },
  {
    title: "Working local inference",
    detail:
      "A real model has to be downloaded, digest-checked, loaded, and used to generate a response. No model has ever been loaded.",
  },
  {
    title: "A checksum taken from the file",
    detail:
      "The SHA-256 has to be computed from the exact installer being published, not written in advance.",
  },
  {
    title: "An approved release identity",
    detail:
      "A release version and a signing identity have to be approved and, if signing is used, verified on the artifact itself.",
  },
  {
    title: "Release notes and a support route",
    detail:
      "Notes describing what the release contains, and a support contact, have to exist before users are pointed at a download.",
  },
];
