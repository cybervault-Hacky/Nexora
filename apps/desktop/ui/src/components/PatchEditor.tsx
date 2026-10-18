import { Icon } from "./Icon";

interface Props {
  path: string;
  value: string;
  onChange: (value: string) => void;
  onCancel: () => void;
  onPreview: () => void;
  disabled?: boolean;
}

export function PatchEditor({ path, value, onChange, onCancel, onPreview, disabled }: Props) {
  return (
    <div className="patch-editor" aria-label="Patch editor">
      <header className="patch-editor__header">
        <div className="patch-editor__title">
          <Icon name="edit" size={13} />
          <span title={path}>Editing: {path}</span>
        </div>
        <button type="button" className="patch-editor__close" onClick={onCancel} aria-label="Cancel editing">
          <Icon name="x" size={13} />
        </button>
      </header>

      <p className="patch-editor__note">Edit the proposed new content. The original file will not be changed until you preview and explicitly apply. Generated content is untrusted until validated.</p>

      <textarea
        className="patch-editor__input"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        rows={14}
        spellCheck={false}
        aria-label={`Proposed content for ${path}`}
        placeholder="Edit proposed file content…"
      />

      <div className="patch-editor__actions">
        <button type="button" className="button button--ghost" onClick={onCancel}>
          Cancel
        </button>
        <button type="button" className="button button--primary" onClick={onPreview} disabled={disabled}>
          Preview patch
        </button>
      </div>
    </div>
  );
}
