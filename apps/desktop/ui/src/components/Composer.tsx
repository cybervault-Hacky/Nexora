import {
  useEffect,
  useLayoutEffect,
  type FormEvent,
  type RefObject,
} from "react";
import {
  MAX_MESSAGE_LENGTH,
  WARN_MESSAGE_LENGTH,
  type ComposerError,
} from "../features/chat/chat";
import { Icon } from "./Icon";

const MAX_TEXTAREA_HEIGHT = 208;

interface ComposerProps {
  value: string;
  error: ComposerError | null;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  onChange: (value: string) => void;
  onSubmit: () => void;
  onDismissError: () => void;
}

export function Composer({
  value,
  error,
  textareaRef,
  onChange,
  onSubmit,
  onDismissError,
}: ComposerProps) {
  const length = value.length;
  const trimmedLength = value.trim().length;
  const tooLong = length > MAX_MESSAGE_LENGTH;
  const showCounter = length > WARN_MESSAGE_LENGTH;
  const canSend = trimmedLength > 0 && !tooLong;

  useLayoutEffect(() => {
    const textarea = textareaRef.current;

    if (!textarea) {
      return;
    }

    textarea.style.height = "auto";
    textarea.style.height = `${Math.min(textarea.scrollHeight, MAX_TEXTAREA_HEIGHT)}px`;
  }, [value, textareaRef]);

  useEffect(() => {
    if (error) {
      textareaRef.current?.focus();
    }
  }, [error, textareaRef]);

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSubmit();
  }

  return (
    <form className="composer-wrap" onSubmit={handleSubmit} aria-label="Message composer">
      <div className={`composer ${error ? "composer--error" : ""}`}>
        <label className="sr-only" htmlFor="composer-input">
          Message Nexora
        </label>

        <textarea
          id="composer-input"
          ref={textareaRef}
          className="composer__input"
          value={value}
          rows={1}
          placeholder="Describe what you want to build…"
          aria-describedby="composer-help composer-status"
          aria-invalid={error ? true : undefined}
          spellCheck
          onChange={(event) => {
            onChange(event.target.value);
            if (error) {
              onDismissError();
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Escape" && error) {
              event.preventDefault();
              onDismissError();
              return;
            }

            if (
              event.key === "Enter" &&
              !event.shiftKey &&
              !event.nativeEvent.isComposing
            ) {
              event.preventDefault();
              onSubmit();
            }
          }}
        />

        <div className="composer__footer">
          <p className="composer__status" id="composer-status">
            <span className="status-dot status-dot--idle" aria-hidden="true" />
            <span>No model connected — replies are scaffold text</span>
          </p>

          <div className="composer__actions">
            {showCounter ? (
              <span
                className={`composer__counter ${tooLong ? "composer__counter--danger" : ""}`}
                aria-hidden="true"
              >
                {length.toLocaleString("en-US")} /{" "}
                {MAX_MESSAGE_LENGTH.toLocaleString("en-US")}
              </span>
            ) : null}

            <button
              type="submit"
              className="send"
              disabled={!canSend}
              aria-label="Send message"
              aria-keyshortcuts="Enter"
              title={canSend ? "Send (Enter)" : "Type a message to send"}
            >
              <Icon name="arrow-up" size={17} />
            </button>
          </div>
        </div>
      </div>

      {error ? (
        <p className="composer__error" role="alert">
          <Icon name="alert" size={14} />
          {error.message}
        </p>
      ) : null}

      <p className="composer__help" id="composer-help">
        Enter to send · Shift + Enter for a new line · Ctrl + L to focus
      </p>
    </form>
  );
}
