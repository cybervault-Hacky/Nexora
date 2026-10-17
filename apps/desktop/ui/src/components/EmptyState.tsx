import { ApertureMark } from "./ApertureMark";

const SUGGESTIONS = [
  {
    title: "Build a landing page",
    prompt: "Build a responsive landing page for a small studio, with a hero and pricing.",
  },
  {
    title: "Refactor a component",
    prompt: "Refactor this React component to separate data fetching from presentation.",
  },
  {
    title: "Explain an error",
    prompt: "Explain this TypeScript error and suggest the smallest safe fix.",
  },
];

interface EmptyStateProps {
  onPick: (prompt: string) => void;
}

export function EmptyState({ onPick }: EmptyStateProps) {
  return (
    <div className="empty">
      <div className="empty__mark" aria-hidden="true">
        <ApertureMark size={58} />
      </div>

      <p className="eyebrow">Local-first AI coding</p>
      <h1 className="empty__title">Build something real.</h1>
      <p className="empty__copy">
        Nexora is a desktop studio for planning, writing, and checking code with a
        model that runs on your machine. This first build contains the interface
        shell only — the local engine is not connected yet.
      </p>

      <div className="empty__suggestions" aria-label="Example prompts">
        {SUGGESTIONS.map((suggestion) => (
          <button
            key={suggestion.title}
            type="button"
            className="suggestion"
            onClick={() => onPick(suggestion.prompt)}
          >
            <span className="suggestion__title">{suggestion.title}</span>
            <span className="suggestion__prompt">{suggestion.prompt}</span>
          </button>
        ))}
      </div>

      <p className="empty__note">
        No account and no cloud inference. The packaged app makes no network
        request to render this window.
      </p>
    </div>
  );
}
