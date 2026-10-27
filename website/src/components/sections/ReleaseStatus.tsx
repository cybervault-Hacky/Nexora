import "./ReleaseStatus.css";

export interface ReleaseStatusProps {
  /** True only when a real, published installer exists. */
  available: boolean;
}

/**
 * Prominent release-status banner.
 *
 * When nothing is published this is plain text, deliberately **not** a disabled
 * button: a greyed-out "Download" control still reads as a control that ought to
 * work, and invites clicking. Text cannot be mistaken for a working download.
 */
export default function ReleaseStatus({ available }: ReleaseStatusProps) {
  if (available) {
    // Rendered only once a verified release is wired up in `content/release.ts`.
    return (
      <div className="release-status release-status--available" role="status">
        <p className="release-status__label">Available</p>
        <p className="release-status__text">
          The current Windows release is listed below.
        </p>
      </div>
    );
  }

  return (
    <div className="release-status" role="status">
      <p className="release-status__label">
        <span className="release-status__dot" aria-hidden="true" />
        Not available yet
      </p>
      <p className="release-status__text">
        There is no Windows installer to download. This page will publish the
        download, and the release details to check it against, once a genuine
        verified release exists.
      </p>
    </div>
  );
}
