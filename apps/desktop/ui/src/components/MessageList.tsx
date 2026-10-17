import type { ChatMessage } from "../features/chat/chat";
import { ApertureMark } from "./ApertureMark";

interface MessageListProps {
  messages: ChatMessage[];
}

function formatTime(timestamp: number): string {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  }).format(timestamp);
}

export function MessageList({ messages }: MessageListProps) {
  return (
    <div className="messages" role="list" aria-label="Conversation">
      {messages.map((message) =>
        message.role === "user" ? (
          <article className="message message--user" role="listitem" key={message.id}>
            <div className="message__bubble">
              <p>{message.content}</p>
            </div>
            <span className="message__meta">You · {formatTime(message.createdAt)}</span>
          </article>
        ) : (
          <article className="message message--assistant" role="listitem" key={message.id}>
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

              {message.content.split("\n\n").map((paragraph) => (
                <p key={paragraph}>{paragraph}</p>
              ))}
            </div>
          </article>
        ),
      )}
    </div>
  );
}
