import { useEffect, useRef, useState } from "react";
import type { ConversationSummary } from "../lib/history";
import { Icon } from "./Icon";

interface HistoryListProps {
  conversations: ConversationSummary[];
  activeId: string | null;
  loading: boolean;
  error: string | null;
  collapsed: boolean;
  onSelect: (id: string) => void;
  onRename: (id: string, newTitle: string) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  onRetry: () => void;
}

export function HistoryList({
  conversations,
  activeId,
  loading,
  error,
  collapsed,
  onSelect,
  onRename,
  onDelete,
  onRetry,
}: HistoryListProps) {
  if (collapsed) {
    return null;
  }

  return (
    <div className="history" aria-label="Conversation history">
      <div className="history__header">
        <h2 className="sidebar__label">Recent</h2>
        <span className="history__count" aria-label={`${conversations.length} conversations`}>
          {conversations.length > 0 ? conversations.length : ""}
        </span>
      </div>

      {loading ? (
        <p className="history__empty" role="status">
          Loading conversations…
        </p>
      ) : error ? (
        <div className="history__error" role="alert">
          <Icon name="alert" size={13} />
          <span>{error}</span>
          <button type="button" className="history__retry" onClick={onRetry}>
            Retry
          </button>
        </div>
      ) : conversations.length === 0 ? (
        <p className="sidebar__empty">No conversations yet. Your chats will appear here and persist after restart.</p>
      ) : (
        <ul className="history__list" role="list">
          {conversations.map((conv) => (
            <HistoryItem
              key={conv.id}
              conversation={conv}
              selected={conv.id === activeId}
              onSelect={onSelect}
              onRename={onRename}
              onDelete={onDelete}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

interface HistoryItemProps {
  conversation: ConversationSummary;
  selected: boolean;
  onSelect: (id: string) => void;
  onRename: (id: string, newTitle: string) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
}

function HistoryItem({ conversation, selected, onSelect, onRename, onDelete }: HistoryItemProps) {
  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState(conversation.title);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [editing]);

  useEffect(() => {
    setEditValue(conversation.title);
  }, [conversation.title]);

  const handleSelect = () => {
    if (editing) return;
    onSelect(conversation.id);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !editing) {
      e.preventDefault();
      onSelect(conversation.id);
    }
    if (e.key === "Delete" && !editing && !confirmDelete) {
      e.preventDefault();
      setConfirmDelete(true);
    }
    if (e.key === "Escape" && confirmDelete) {
      setConfirmDelete(false);
    }
  };

  const handleRename = async () => {
    const trimmed = editValue.trim();
    if (trimmed.length === 0) {
      setEditing(false);
      setEditValue(conversation.title);
      return;
    }
    if (trimmed === conversation.title) {
      setEditing(false);
      return;
    }
    setBusy(true);
    try {
      await onRename(conversation.id, trimmed);
      setEditing(false);
    } catch {
      // error is surfaced via parent saveError
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async () => {
    setBusy(true);
    try {
      await onDelete(conversation.id);
      setConfirmDelete(false);
    } catch {
      // keep confirm open on error
    } finally {
      setBusy(false);
    }
  };

  const preview = conversation.preview || `${conversation.messageCount} message${conversation.messageCount !== 1 ? "s" : ""}`;

  return (
    <li
      className={`history__item ${selected ? "history__item--selected" : ""}`}
      role="listitem"
      tabIndex={0}
      aria-selected={selected}
      aria-label={`${conversation.title}, ${conversation.messageCount} messages`}
      onClick={handleSelect}
      onKeyDown={handleKeyDown}
    >
      <div className="history__item-main">
        {editing ? (
          <input
            ref={inputRef}
            className="history__edit-input"
            value={editValue}
            maxLength={200}
            disabled={busy}
            aria-label="Edit conversation title"
            onChange={(e) => setEditValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void handleRename();
              }
              if (e.key === "Escape") {
                e.preventDefault();
                setEditing(false);
                setEditValue(conversation.title);
              }
              e.stopPropagation();
            }}
            onClick={(e) => e.stopPropagation()}
            onBlur={() => {
              // Don't auto-save on blur, require Enter; but reset if empty
              if (editValue.trim() === conversation.title) {
                setEditing(false);
              }
            }}
          />
        ) : (
          <>
            <span className="history__title" title={conversation.title}>
              {conversation.title}
            </span>
            <span className="history__preview" title={preview}>
              {preview}
            </span>
          </>
        )}
        <span className="history__meta">
          {conversation.messageCount > 0 ? `${conversation.messageCount} msg` : "Empty"} ·{" "}
          {formatTime(conversation.updatedAt)}
        </span>
      </div>

      <div className="history__actions" onClick={(e) => e.stopPropagation()}>
        {editing ? (
          <>
            <button
              type="button"
              className="history__action"
              aria-label="Save title"
              title="Save (Enter)"
              disabled={busy}
              onClick={() => void handleRename()}
            >
              <Icon name="check" size={13} />
            </button>
            <button
              type="button"
              className="history__action"
              aria-label="Cancel editing"
              title="Cancel (Esc)"
              disabled={busy}
              onClick={() => {
                setEditing(false);
                setEditValue(conversation.title);
              }}
            >
              <Icon name="close" size={13} />
            </button>
          </>
        ) : confirmDelete ? (
          <>
            <span className="history__confirm-label">Delete?</span>
            <button
              type="button"
              className="history__action history__action--danger"
              aria-label={`Confirm delete ${conversation.title}`}
              disabled={busy}
              onClick={() => void handleDelete()}
            >
              {busy ? "…" : "Yes"}
            </button>
            <button
              type="button"
              className="history__action"
              aria-label="Cancel delete"
              disabled={busy}
              onClick={() => setConfirmDelete(false)}
            >
              No
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              className="history__action"
              aria-label={`Rename ${conversation.title}`}
              title="Rename"
              onClick={() => {
                setEditValue(conversation.title);
                setEditing(true);
              }}
            >
              <Icon name="edit" size={13} />
            </button>
            <button
              type="button"
              className="history__action history__action--danger"
              aria-label={`Delete ${conversation.title}`}
              title="Delete"
              onClick={() => setConfirmDelete(true)}
            >
              <Icon name="trash" size={13} />
            </button>
          </>
        )}
      </div>
    </li>
  );
}

function formatTime(timestamp: number): string {
  try {
    const d = new Date(timestamp);
    const now = new Date();
    if (d.toDateString() === now.toDateString()) {
      return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
    }
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
  } catch {
    return "";
  }
}
