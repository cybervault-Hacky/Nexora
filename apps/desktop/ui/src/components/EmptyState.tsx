import { RouterLink } from "./RouterLink";

interface EmptyStateProps {
  note?: string | null;
  showModelLink?: boolean;
  onPick?: (prompt: string) => void;
}

/** Minimal new-chat welcome; the single composer remains a sibling in ChatPage. */
export function EmptyState({ note, showModelLink = false }: EmptyStateProps) {
  return (
    <div className="empty empty--welcome">
      <h1 className="empty__title">What's New Today?</h1>
      {note && <p className="empty__copy">{note}</p>}
      {showModelLink && <RouterLink to="models" className="empty__model-link">Open Local Models</RouterLink>}
    </div>
  );
}
