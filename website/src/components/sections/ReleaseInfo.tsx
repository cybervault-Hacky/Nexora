import type { ReleaseMetadata } from "../../content/release";

import "./ReleaseInfo.css";

export interface ReleaseInfoProps {
  /** Verified release metadata, or `null` when nothing is published. */
  release: ReleaseMetadata | null;
}

/** Formats a byte count as both an exact value and a rounded GiB/MiB figure. */
function formatSize(bytes: number): string {
  const mib = bytes / (1024 * 1024);
  const gib = mib / 1024;
  const rounded = gib >= 1 ? `${gib.toFixed(2)} GiB` : `${mib.toFixed(0)} MiB`;
  return `${bytes.toLocaleString("en-US")} bytes (${rounded})`;
}

/**
 * Release metadata table.
 *
 * Renders **nothing at all** when no verified release exists. An empty field
 * grid or a row of "—" placeholders would read as missing data that ought to be
 * there; rendering nothing keeps the page honest while the surrounding page
 * still explains what will eventually appear here.
 *
 * Every value is displayed exactly as recorded in `content/release.ts`, which
 * must only be filled from a real published artifact.
 */
export default function ReleaseInfo({ release }: ReleaseInfoProps) {
  if (!release) {
    return null;
  }

  const rows: ReadonlyArray<{ term: string; detail: string; mono?: boolean }> = [
    { term: "Version", detail: release.version },
    { term: "Architecture", detail: release.architecture },
    { term: "Installer file", detail: release.fileName },
    { term: "File size", detail: formatSize(release.sizeBytes) },
    { term: "SHA-256", detail: release.sha256, mono: true },
    { term: "Released", detail: release.releasedAt },
  ];

  return (
    <>
      <h2 className="release-info__title" id="release-information">
        Release information
      </h2>
      <dl className="release-info">
        {rows.map((row) => (
          <div
            className={
              row.mono
                ? "release-info__row release-info__row--mono"
                : "release-info__row"
            }
            key={row.term}
          >
            <dt className="release-info__term">{row.term}</dt>
            <dd className="release-info__detail">{row.detail}</dd>
          </div>
        ))}
      </dl>
      {release.releaseNotesUrl ? (
        <p className="release-info__notes">
          <a href={release.releaseNotesUrl}>Release notes</a>
        </p>
      ) : null}
    </>
  );
}
