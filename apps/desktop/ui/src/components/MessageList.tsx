import { memo } from "react";
import type { ChatMessage } from "../features/chat/chat";
import { AssistantContent, UserContent } from "./MessageContent";

const timeFormatter = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
});

function formatTime(timestamp: number): string {
  return timeFormatter.format(timestamp);
}

const UserMessage = memo(function UserMessage({ message }: { message: ChatMessage }) {
  return (
    <article className="message message--user">
      <div className="message__bubble">
        <UserContent content={message.content} />
        {message.imageReference ? (
          <div className="message__image-reference" aria-label="Validated image reference metadata">
            <span>Image attached · Reference validated</span>
            <span>PNG · {message.imageReference.width} × {message.imageReference.height} · {(message.imageReference.sizeBytes / 1024).toFixed(1)} KB</span>
            {message.imageTo3dTask ? <span>Image-to-3D task prepared · not analyzed</span> : null}
            <span>Opaque local reference retained with this conversation.</span>
          </div>
        ) : null}
      </div>
      <span className="message__meta">
        You · {formatTime(message.createdAt)}
      </span>
    </article>
  );
});

const AssistantMessage = memo(function AssistantMessage({
  message,
}: {
  message: ChatMessage;
}) {
  const hasContent = message.content.trim().length > 0;
  const streaming = message.state === "streaming";

  return (
    <article className="message message--assistant">
      <div className="message__body">
        <div className="message__head">
          <span className="message__author">Nexora</span>

          {streaming ? (
            <span className="chip chip--live">
              <span className="pulse" aria-hidden="true" />
              Generating
            </span>
          ) : null}

          {message.state === "stopped" ? (
            <span className="chip chip--warning">Stopped</span>
          ) : null}

          {message.state === "error" ? (
            <span className="chip chip--unknown">Failed</span>
          ) : null}

          <span className="message__meta">{formatTime(message.createdAt)}</span>
        </div>

        {hasContent ? (
          <AssistantContent content={message.content} streaming={streaming} />
        ) : message.state === "stopped" ? (
          <p className="message__pending">Stopped before any text arrived.</p>
        ) : (
          <p className="message__pending" aria-live="polite">
            Waiting for the first token…
          </p>
        )}

        {message.failure ? (
          <p className="message__failure">{message.failure}</p>
        ) : null}
      </div>
    </article>
  );
});

interface MessageListProps {
  messages: ChatMessage[];
}

export function MessageList({ messages }: MessageListProps) {
  return (
    <div
      className="messages"
      role="log"
      aria-live="polite"
      aria-relevant="additions"
      aria-label="Conversation"
    >
      {messages.map((message) =>
        message.role === "user" ? (
          <UserMessage key={message.id} message={message} />
        ) : (
          <AssistantMessage key={message.id} message={message} />
        ),
      )}
    </div>
  );
}
