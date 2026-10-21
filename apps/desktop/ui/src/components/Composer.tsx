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
  /** True while a model reply is streaming. */
  busy: boolean;
  /** False when no model is loaded, so sending is disabled with a reason. */
  canGenerate: boolean;
  /**
   * Why sending is blocked when the engine itself is ready — today the daily
   * allowance. Null/undefined keeps the engine wording.
   */
  blockedReason?: string | null;
  onChange: (value: string) => void;
  onSubmit: () => void;
  onStop: () => void;
  onDismissError: () => void;
}

export function Composer({
  value,
  error,
  textareaRef,
  busy,
  canGenerate,
  blockedReason = null,
  onChange,
  onSubmit,
  onStop,
  onDismissError,
}: ComposerProps) {
  const length = value.length;
  const trimmedLength = value.trim().length;
  const tooLong = length > MAX_MESSAGE_LENGTH;
  const showCounter = length > WARN_MESSAGE_LENGTH;
  const canSend = canGenerate && trimmedLength > 0 && !tooLong && !busy;

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

  function requestSubmit() {
    if (!canGenerate || busy) {
      return;
    }

    onSubmit();
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    requestSubmit();
  }

  const statusText = busy
    ? "Generating on this machine…"
    : canGenerate
      ? "Local model ready"
      : (blockedReason ?? "No model loaded");

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
          placeholder={
            canGenerate
              ? "Describe what you want to build…"
              : (blockedReason ?? "Load a local model to start")
          }
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

            if (event.key === "Escape" && busy) {
              event.preventDefault();
              onStop();
              return;
            }

            if (
              event.key === "Enter" &&
              !event.shiftKey &&
              !event.nativeEvent.isComposing
            ) {
              event.preventDefault();
              requestSubmit();
            }
          }}
        />

        <div className="composer__footer">
          <p className="composer__status" id="composer-status">
            <span
              className={`status-dot ${busy ? "status-dot--busy" : canGenerate ? "status-dot--ok" : "status-dot--idle"}`}
              aria-hidden="true"
            />
            <span>{statusText}</span>
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

            {busy ? (
              <button
                type="button"
                className="send send--stop"
                onClick={onStop}
                aria-label="Stop generating"
                aria-keyshortcuts="Escape"
                title="Stop (Esc)"
              >
                <Icon name="stop" size={14} />
              </button>
            ) : (
              <button
                type="submit"
                className="send"
                disabled={!canSend}
                aria-label="Send message"
                aria-keyshortcuts="Enter"
                title={
                  canGenerate
                    ? canSend
                      ? "Send (Enter)"
                      : "Type a message to send"
                    : (blockedReason ?? "Load a model to send")
                }
              >
                <Icon name="arrow-up" size={17} />
              </button>
            )}
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
        Enter to send · Shift + Enter for a new line · Esc to stop · Ctrl + L to focus
      </p>
    </form>
  );
}
