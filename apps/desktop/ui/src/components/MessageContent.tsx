import { Fragment, memo, useMemo } from "react";
import { parseInline, parseMessage, type InlineSegment } from "../lib/code";
import { CodeBlock } from "./CodeBlock";

function Inline({ segments }: { segments: InlineSegment[] }) {
  return (
    <>
      {segments.map((segment, index) =>
        segment.code ? (
          <code className="inline-code" key={`c${index}`}>
            {segment.text}
          </code>
        ) : (
          <Fragment key={`t${index}`}>{segment.text}</Fragment>
        ),
      )}
    </>
  );
}

/** One text paragraph. Separate component so its parse hook is unconditional. */
function Paragraph({ text, caret }: { text: string; caret: boolean }) {
  const segments = useMemo(() => parseInline(text), [text]);

  return (
    <p>
      <Inline segments={segments} />
      {caret ? <span className="caret" aria-hidden="true" /> : null}
    </p>
  );
}

interface MessageContentProps {
  content: string;
  /** Shows the trailing caret while tokens are still arriving. */
  streaming?: boolean;
}

/**
 * Renders an assistant reply as paragraphs, inline code, and fenced code.
 *
 * Every model-produced character becomes a text node: there is no HTML path, so
 * a reply containing `<script>` or an `<img onerror=...>` is displayed literally
 * and cannot execute.
 */
export const AssistantContent = memo(function AssistantContent({
  content,
  streaming = false,
}: MessageContentProps) {
  const blocks = useMemo(() => parseMessage(content), [content]);

  return (
    <>
      {blocks.map((block, index) => {
        const last = index === blocks.length - 1;

        if (block.kind === "code") {
          return (
            <div className="message__block" key={`b${index}`}>
              <CodeBlock
                code={block.code}
                language={block.language}
                closed={block.closed}
              />
              {streaming && last ? <span className="caret" aria-hidden="true" /> : null}
            </div>
          );
        }

        return (
          <Paragraph
            key={`b${index}`}
            text={block.text}
            caret={streaming && last}
          />
        );
      })}
    </>
  );
});

/** A user prompt keeps its own line breaks verbatim; it is never parsed. */
export const UserContent = memo(function UserContent({ content }: { content: string }) {
  return <p className="message__prompt">{content}</p>;
});
