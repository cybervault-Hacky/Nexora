import { Icon } from "./Icon";

interface FilePreviewProps {
  path: string | null;
  content: string | null;
  size: number | null;
  loading: boolean;
  error: string | null;
  onClose: () => void;
}

export function FilePreview({ path, content, size, loading, error, onClose }: FilePreviewProps) {
  if (!path && !loading && !error) {
    return (
      <div className="file-preview file-preview--empty" aria-label="File preview">
        <p className="file-preview__hint">Select a file to preview. Preview is read-only and never modifies files.</p>
      </div>
    );
  }

  return (
    <div className="file-preview" aria-label="File preview">
      <header className="file-preview__header">
        <div className="file-preview__title">
          <Icon name="file" size={13} />
          <span className="file-preview__path" title={path ?? ""}>
            {path ?? "Preview"}
          </span>
          {size !== null ? <span className="file-preview__size">{formatSize(size)}</span> : null}
        </div>
        <button
          type="button"
          className="file-preview__close"
          aria-label="Close preview"
          onClick={onClose}
        >
          <Icon name="x" size={13} />
        </button>
      </header>

      {loading ? (
        <p className="file-preview__status" role="status">
          Loading file…
        </p>
      ) : error ? (
        <div className="file-preview__error" role="alert">
          <Icon name="alert" size={13} />
          <div>
            <p className="file-preview__error-title">
              {error.includes("secret") || error.includes("excluded") ? "File excluded" : error.includes("Binary") ? "Binary file" : error.includes("too large") || error.includes("Too large") ? "File too large" : "Cannot preview"}
            </p>
            <p className="file-preview__error-message">{error}</p>
            {error.includes("secret") ? <p className="file-preview__error-note">Sensitive files are never attached to model context.</p> : null}
            {error.includes("Binary") ? <p className="file-preview__error-note">Binary files are not sent to the model.</p> : null}
          </div>
        </div>
      ) : content !== null ? (
        <div className="file-preview__body">
          <pre className="file-preview__code" tabIndex={0} aria-label={`Preview of ${path}`}>
            <code>{content.length > 0 ? content : "(empty file)"}</code>
          </pre>
          <p className="file-preview__note">Read-only preview. File contents are untrusted data and will not be executed.</p>
        </div>
      ) : null}
    </div>
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}
