import { useEffect, useId, useRef, useState } from "react";
import { Icon } from "./Icon";
import {
  filterSummariesLocally,
  searchLocalConversations,
  type ConversationSearchHit,
} from "../features/chat/searchConversations";
import type { ConversationSummary } from "../lib/history";

interface ConversationSearchModalProps {
  open: boolean;
  conversations: readonly ConversationSummary[];
  activeId: string | null;
  historyLoading: boolean;
  historyError: string | null;
  onClose: () => void;
  onSelectConversation: (id: string) => void;
  onNewChat: () => void;
  onRetryHistory: () => void;
}

function formatUpdatedDate(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return "";
  try {
    return new Date(timestamp).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
  } catch {
    return "";
  }
}

export function ConversationSearchModal({
  open,
  conversations,
  activeId,
  historyLoading,
  historyError,
  onClose,
  onSelectConversation,
  onNewChat,
  onRetryHistory,
}: ConversationSearchModalProps) {
  const titleId = useId();
  const listboxId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const requestSeqRef = useRef(0);

  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ConversationSearchHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [selectedIndex, setSelectedIndex] = useState(0);

  // Reset modal transient search input each time it opens while preserving chat state.
  useEffect(() => {
    if (!open) return;
    setQuery("");
    setSearchError(null);
    setSearching(false);
    setHits(filterSummariesLocally(conversations, ""));
    setSelectedIndex(0);

    const frame = window.requestAnimationFrame(() => {
      inputRef.current?.focus();
    });
    return () => window.cancelAnimationFrame(frame);
  }, [open]);

  // Execute local search with monotonic sequence IDs so stale async lookups cannot win.
  useEffect(() => {
    if (!open) return;

    const seq = ++requestSeqRef.current;
    let cancelled = false;

    const trimmed = query.trim();
    if (trimmed.length === 0) {
      setSearching(false);
      setSearchError(null);
      setHits(filterSummariesLocally(conversations, ""));
      setSelectedIndex(0);
      return;
    }

    // Immediate synchronous summary pass so titles/previews respond immediately.
    const immediate = filterSummariesLocally(conversations, trimmed);
    setHits(immediate);
    setSelectedIndex(0);
    setSearching(true);
    setSearchError(null);

    void searchLocalConversations(conversations, trimmed, () => cancelled || seq !== requestSeqRef.current)
      .then((resolved) => {
        if (cancelled || seq !== requestSeqRef.current) return;
        setHits(resolved);
        setSelectedIndex((prev) =>
          resolved.length === 0 ? 0 : Math.min(prev, resolved.length - 1),
        );
        setSearching(false);
      })
      .catch((err: unknown) => {
        if (cancelled || seq !== requestSeqRef.current) return;
        const msg =
          typeof err === "object" && err !== null && "message" in err
            ? String((err as { message: unknown }).message)
            : "Could not search local conversations.";
        setSearchError(msg);
        setSearching(false);
      });

    return () => {
      cancelled = true;
    };
  }, [open, query, conversations]);

  useEffect(() => {
    if (!open) return;
    const onWindowKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onWindowKeyDown);
    return () => window.removeEventListener("keydown", onWindowKeyDown);
  }, [open, onClose]);

  if (!open) return null;

  const effectiveError = historyError ?? searchError;

  const handleChoose = (id: string) => {
    onSelectConversation(id);
    onClose();
  };

  const handleInputKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (hits.length > 0) {
        setSelectedIndex((prev) => (prev + 1) % hits.length);
      }
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      if (hits.length > 0) {
        setSelectedIndex((prev) => (prev - 1 + hits.length) % hits.length);
      }
      return;
    }
    if (event.key === "Enter") {
      const target = hits[selectedIndex];
      if (target) {
        event.preventDefault();
        handleChoose(target.conversation.id);
      }
    }
  };

  const activeOptionId =
    hits[selectedIndex] ? `${listboxId}-opt-${hits[selectedIndex]!.conversation.id}` : undefined;

  return (
    <div
      className="search-modal-backdrop"
      role="presentation"
      data-testid="conversation-search-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <div
        className="search-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        data-testid="conversation-search-modal"
      >
        <div className="search-modal__top">
          <Icon name="search" size={16} className="search-modal__search-icon" />
          <label htmlFor={`${titleId}-input`} id={titleId} className="visually-hidden">
            Search conversations
          </label>
          <input
            ref={inputRef}
            id={`${titleId}-input`}
            className="search-modal__input"
            type="search"
            role="combobox"
            aria-expanded="true"
            aria-controls={listboxId}
            aria-activedescendant={activeOptionId}
            aria-label="Search conversations"
            placeholder="Search chats by title or local messages…"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={handleInputKeyDown}
            autoComplete="off"
          />
          {query.length > 0 && (
            <button
              type="button"
              className="search-modal__clear"
              aria-label="Clear search query"
              onClick={() => {
                setQuery("");
                inputRef.current?.focus();
              }}
            >
              Clear
            </button>
          )}
          <button
            type="button"
            className="search-modal__close"
            aria-label="Close conversation search"
            onClick={onClose}
          >
            <Icon name="x" size={16} />
          </button>
        </div>

        <div className="search-modal__quick-row">
          <button
            type="button"
            className="search-modal__new-chat"
            onClick={() => {
              onNewChat();
              onClose();
            }}
          >
            <Icon name="edit" size={14} />
            <span>New chat</span>
          </button>
          {searching && (
            <span className="search-modal__status" role="status">
              Searching local history…
            </span>
          )}
        </div>

        <div className="search-modal__body">
          {effectiveError ? (
            <div className="search-modal__error" role="alert">
              <Icon name="alert" size={15} />
              <span>{effectiveError}</span>
              <button
                type="button"
                className="button button--ghost button--sm"
                onClick={onRetryHistory}
              >
                Retry
              </button>
            </div>
          ) : historyLoading && conversations.length === 0 ? (
            <p className="search-modal__empty" role="status">
              Loading local conversations…
            </p>
          ) : hits.length === 0 ? (
            <div className="search-modal__empty" data-testid="search-empty-state">
              {query.trim().length > 0 ? (
                <>
                  <p className="search-modal__empty-title">
                    No local conversations match &ldquo;{query.trim()}&rdquo;
                  </p>
                  <p className="search-modal__empty-hint">
                    Try another keyword from a conversation title or saved message.
                  </p>
                </>
              ) : (
                <>
                  <p className="search-modal__empty-title">No conversations yet</p>
                  <p className="search-modal__empty-hint">
                    Conversations saved on this device will appear here for quick search.
                  </p>
                </>
              )}
            </div>
          ) : (
            <ul
              id={listboxId}
              className="search-modal__results"
              role="listbox"
              aria-label={query.trim().length > 0 ? "Matching conversations" : "Recent conversations"}
            >
              <li className="search-modal__section-label" role="presentation">
                {query.trim().length > 0
                  ? `${hits.length} matching ${hits.length === 1 ? "conversation" : "conversations"}`
                  : "Recent conversations"}
              </li>
              {hits.map((hit, idx) => {
                const isSelected = idx === selectedIndex;
                const isCurrent = hit.conversation.id === activeId;
                return (
                  <li
                    key={hit.conversation.id}
                    id={`${listboxId}-opt-${hit.conversation.id}`}
                    role="option"
                    aria-selected={isSelected}
                    className={`search-modal__option${isSelected ? " search-modal__option--active" : ""}${isCurrent ? " search-modal__option--current" : ""}`}
                    onMouseEnter={() => setSelectedIndex(idx)}
                    onClick={() => handleChoose(hit.conversation.id)}
                  >
                    <Icon name="message" size={15} className="search-modal__option-icon" />
                    <div className="search-modal__option-main">
                      <div className="search-modal__option-row">
                        <span className="search-modal__option-title">
                          {hit.conversation.title}
                        </span>
                        <span className="search-modal__option-date">
                          {hit.matchSource === "message"
                            ? "Message match"
                            : formatUpdatedDate(hit.conversation.updatedAt)}
                        </span>
                      </div>
                      {hit.snippet ? (
                        <span className="search-modal__option-snippet">{hit.snippet}</span>
                      ) : null}
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        <div className="search-modal__footer">
          <span>↑↓ Navigate · Enter Open · Esc Close</span>
          <span>Local-only search on this device</span>
        </div>
      </div>
    </div>
  );
}
