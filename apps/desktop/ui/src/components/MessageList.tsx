import { memo } from "react";
import type { ChatMessage } from "../features/chat/chat";
import { ApertureMark } from "./ApertureMark";

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
        <p>{message.content}</p>
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
  const paragraphs = message.content.split("\n\n");

  return (
    <article className="message message--assistant">
      <span className="message__mark" aria-hidden="true">
        <ApertureMark size={22} />
      </span>

      <div className="message__body">
        <div className="message__head">
          <span className="message__author">Nexora</span>
          {message.kind === "scaffold" ? (
            <span className="chip chip--warning">Scaffold · no model</span>
          ) : null}
          <span className="message__meta">{formatTime(message.createdAt)}</span>
        </div>

        {paragraphs.map((paragraph, index) => (
          <p key={`${message.id}-p${index}`}>{paragraph}</p>
        ))}
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
