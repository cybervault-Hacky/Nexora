import {
  useEffect,
  useLayoutEffect,
  useRef,
  type FormEvent,
  type ReactNode,
  type RefObject,
} from "react";
import {
  MAX_MESSAGE_LENGTH,
  WARN_MESSAGE_LENGTH,
  type ComposerError,
} from "../features/chat/chat";
import { Icon } from "./Icon";
import type { ComposerMode } from "../features/chat/mediaMode";
import type { ImageReferenceMetadata } from "../features/chat/imageTo3dTask";

const MAX_TEXTAREA_HEIGHT = 208;

interface ComposerProps {
  value: string;
  error: ComposerError | null;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  /** True while a model reply is streaming. */
  busy: boolean;
  /** False when no model is loaded or local daily allowance is exhausted. */
  canGenerate: boolean;
  /**
   * Why sending is blocked when the engine itself is ready — today the daily
   * allowance or a model mismatch. Null/undefined keeps the engine wording.
   */
  blockedReason?: string | null;
  /** The existing approved model selector, rendered inside the composer. */
  modelSelector?: ReactNode;
  /** Optional local media-mode selector; omitted by existing standalone uses. */
  mode?: ComposerMode;
  onModeChange?: (mode: ComposerMode) => void;
  /** Media status is being queried; no model generation is in progress. */
  checkingMedia?: boolean;
  /** A clear local media request can be submitted without a coding model. */
  allowSubmitWithoutModel?: boolean;
  imageReference?: { metadata: ImageReferenceMetadata; previewUrl: string } | null;
  imageReferenceBusy?: boolean;
  imageReferenceError?: string | null;
  onPickImageReference?: (file: File) => void;
  onRemoveImageReference?: () => void;
  onOpenProjects?: () => void;
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
  onOpenProjects,
  modelSelector,
  mode = "auto",
  onModeChange,
  checkingMedia = false,
  allowSubmitWithoutModel = false,
  imageReference = null,
  imageReferenceBusy = false,
  imageReferenceError = null,
  onPickImageReference,
  onRemoveImageReference,
  onChange,
  onSubmit,
  onStop,
  onDismissError,
}: ComposerProps) {
  const imageInputRef = useRef<HTMLInputElement | null>(null);
  const length = value.length;
  const trimmedLength = value.trim().length;
  const tooLong = length > MAX_MESSAGE_LENGTH;
  const showCounter = length > WARN_MESSAGE_LENGTH;
  const canSend =
    (canGenerate || allowSubmitWithoutModel) &&
    trimmedLength > 0 &&
    !tooLong &&
    !busy &&
    !checkingMedia;

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
    if ((!canGenerate && !allowSubmitWithoutModel) || busy || checkingMedia) {
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
    : imageReferenceBusy
      ? "Validating the image reference locally…"
      : imageReference
        ? "Image attached · Reference validated · Analysis unavailable"
        : checkingMedia
          ? "Checking local media capability…"
          : allowSubmitWithoutModel
            ? "Media request — local status queried on send"
            : mode !== "auto"
              ? `${mode === "image" ? "Image" : "Video"} mode — local status queried on send`
              : canGenerate
                ? "Local model ready"
                : (blockedReason ?? "No model loaded");

  return (
    <form className="composer-wrap" onSubmit={handleSubmit} aria-label="Message composer">
      {onPickImageReference ? (
        <input
          ref={imageInputRef}
          className="sr-only"
          type="file"
          accept="image/png,.png"
          aria-label="Choose PNG image reference"
          onChange={(event) => {
            const file = event.currentTarget.files?.[0];
            event.currentTarget.value = "";
            if (file) onPickImageReference(file);
          }}
        />
      ) : null}
      {imageReference ? (
        <div className="composer__reference" role="status" aria-label="Image reference preview">
          <img src={imageReference.previewUrl} alt="Selected image reference preview" />
          <div className="composer__reference-details">
            <strong>Image attached · Reference validated</strong>
            <span>PNG · {imageReference.metadata.width} × {imageReference.metadata.height} · {(imageReference.metadata.sizeBytes / 1024).toFixed(1)} KB</span>
            <span>Analysis unavailable. Image stays local and is not sent to a model.</span>
          </div>
          <button
            type="button"
            className="composer__reference-remove"
            aria-label="Remove image reference"
            disabled={imageReferenceBusy || busy || checkingMedia}
            onClick={onRemoveImageReference}
          >
            <Icon name="x" size={14} />
          </button>
        </div>
      ) : null}
      {imageReferenceError ? <p className="composer__reference-error" role="alert">{imageReferenceError}</p> : null}
      <div className={`composer ${error ? "composer--error" : ""}`}>
        <label className="sr-only" htmlFor="composer-input">
          Message Nexora
        </label>

        {onOpenProjects && (
          <button type="button" className="composer__attach" aria-label="Add project files" title="Choose project files" onClick={onOpenProjects}>
            <Icon name="plus" size={19} />
          </button>
        )}

        <textarea
          id="composer-input"
          ref={textareaRef}
          className="composer__input"
          value={value}
          rows={1}
          placeholder={
            imageReference
              ? "Describe the 3D asset you want from this reference…"
              : allowSubmitWithoutModel
                ? "Describe the local media task…"
                : mode === "image"
                  ? "Describe an image task…"
                  : mode === "video"
                    ? "Describe a video task…"
                    : canGenerate
                      ? "Ask anything…"
                      : (blockedReason ?? "Load a local model to start")
          }
          disabled={checkingMedia}
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
          {onPickImageReference ? (
            <button
              type="button"
              className="composer__reference-pick"
              disabled={imageReferenceBusy || busy || checkingMedia}
              onClick={() => imageInputRef.current?.click()}
            >
              <Icon name="file" size={13} />
              {imageReference ? "Replace image" : "Add image reference"}
            </button>
          ) : null}
          {onModeChange ? (
            <div className="composer__mode">
              <label className="composer__mode-label" htmlFor="composer-mode">Mode</label>
              <select
                id="composer-mode"
                className="composer__mode-control"
                aria-label="Chat mode"
                value={mode}
                disabled={busy || checkingMedia}
                onChange={(event) => onModeChange(event.target.value as ComposerMode)}
              >
                <option value="auto">Auto</option>
                <option value="image">Image</option>
                <option value="video">Video</option>
              </select>
            </div>
          ) : null}
          {modelSelector ? <div className="composer__model">{modelSelector}</div> : null}
          <p className="composer__status" id="composer-status">
            <span
              className={`status-dot ${busy ? "status-dot--busy" : canGenerate && mode === "auto" && !allowSubmitWithoutModel ? "status-dot--ok" : "status-dot--idle"}`}
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
                  canGenerate || allowSubmitWithoutModel
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
        Enter to send · Shift + Enter for a new line · Esc to stop · Ctrl + K to search
      </p>
    </form>
  );
}
