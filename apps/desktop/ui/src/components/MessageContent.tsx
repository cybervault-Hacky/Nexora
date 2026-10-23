import { Fragment, memo, useMemo } from "react";
import {
  parseRichInline,
  parseRichMessage,
  type RichInlineNode,
} from "../lib/code";
import { CodeBlock } from "./CodeBlock";

function Inline({ nodes }: { nodes: RichInlineNode[] }) {
  return (
    <>
      {nodes.map((node, index) => {
        switch (node.kind) {
          case "code":
            return (
              <code className="inline-code" key={`c${index}`}>
                {node.text}
              </code>
            );
          case "strong":
            return <strong key={`s${index}`}>{node.text}</strong>;
          case "em":
            return <em key={`e${index}`}>{node.text}</em>;
          case "link":
            return (
              <a
                className="markdown-link"
                href={node.href}
                target="_blank"
                rel="noopener noreferrer"
                key={`l${index}`}
              >
                {node.label}
              </a>
            );
          case "text":
            return <Fragment key={`t${index}`}>{node.text}</Fragment>;
        }
      })}
    </>
  );
}

function RichSpan({ text, caret = false }: { text: string; caret?: boolean }) {
  const nodes = useMemo(() => parseRichInline(text), [text]);
  return (
    <>
      <Inline nodes={nodes} />
      {caret ? <span className="caret" aria-hidden="true" /> : null}
    </>
  );
}

interface MessageContentProps {
  content: string;
  /** Shows the trailing caret while tokens are still arriving. */
  streaming?: boolean;
}

/**
 * Renders an assistant reply cleanly in ChatGPT-like Markdown hierarchy:
 * headings, lists, tables, blockquotes, links, inline code, and fenced code.
 *
 * Every model-produced character resolves strictly to text nodes or validated
 * `http(s)` links — raw HTML (`<script>`, `<img onerror=...>`) cannot execute.
 */
export const AssistantContent = memo(function AssistantContent({
  content,
  streaming = false,
}: MessageContentProps) {
  const blocks = useMemo(() => parseRichMessage(content), [content]);

  return (
    <>
      {blocks.map((block, index) => {
        const last = index === blocks.length - 1;
        const showCaret = streaming && last;

        switch (block.kind) {
          case "code":
            return (
              <div className="message__block" key={`b${index}`}>
                <CodeBlock
                  code={block.code}
                  language={block.language}
                  closed={block.closed}
                />
                {showCaret ? <span className="caret" aria-hidden="true" /> : null}
              </div>
            );

          case "heading": {
            const Tag = `h${block.level}` as "h1" | "h2" | "h3" | "h4" | "h5" | "h6";
            return (
              <Tag
                className={`markdown-heading markdown-heading--${block.level}`}
                key={`b${index}`}
              >
                <RichSpan text={block.text} caret={showCaret} />
              </Tag>
            );
          }

          case "list": {
            if (block.ordered) {
              return (
                <ol className="markdown-list markdown-list--ordered" start={block.start} key={`b${index}`}>
                  {block.items.map((item, itemIdx) => (
                    <li key={`li${itemIdx}`}>
                      <RichSpan
                        text={item}
                        caret={showCaret && itemIdx === block.items.length - 1}
                      />
                    </li>
                  ))}
                </ol>
              );
            }
            return (
              <ul className="markdown-list" key={`b${index}`}>
                {block.items.map((item, itemIdx) => (
                  <li key={`li${itemIdx}`}>
                    <RichSpan
                      text={item}
                      caret={showCaret && itemIdx === block.items.length - 1}
                    />
                  </li>
                ))}
              </ul>
            );
          }

          case "table":
            return (
              <div className="message__block markdown-table-wrap" key={`b${index}`}>
                <table className="markdown-table">
                  <thead>
                    <tr>
                      {block.headers.map((header, colIdx) => (
                        <th scope="col" key={`th${colIdx}`}>
                          <RichSpan text={header} />
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {block.rows.map((row, rowIdx) => (
                      <tr key={`tr${rowIdx}`}>
                        {row.map((cell, colIdx) => (
                          <td key={`td${rowIdx}-${colIdx}`}>
                            <RichSpan text={cell} />
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
                {showCaret ? <span className="caret" aria-hidden="true" /> : null}
              </div>
            );

          case "blockquote":
            return (
              <blockquote className="markdown-quote" key={`b${index}`}>
                <RichSpan text={block.text} caret={showCaret} />
              </blockquote>
            );

          case "hr":
            return <hr className="markdown-hr" key={`b${index}`} />;

          case "paragraph":
            return (
              <p key={`b${index}`}>
                <RichSpan text={block.text} caret={showCaret} />
              </p>
            );
        }
      })}
    </>
  );
});

/** A user prompt keeps its own line breaks verbatim; it is never parsed. */
export const UserContent = memo(function UserContent({ content }: { content: string }) {
  return <p className="message__prompt">{content}</p>;
});
