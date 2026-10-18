import { useCallback, useEffect, useRef, useState } from "react";
import { copyText } from "../lib/clipboard";

/** How long the "Copied" confirmation stays visible. */
const FEEDBACK_MS = 1_600;

type CopyState = "idle" | "copied" | "failed";

interface CodeBlockProps {
  code: string;
  /** Fence language; absent when the model wrote a bare fence. */
  language?: string | null;
  /** False while the closing fence has not arrived during streaming. */
  closed?: boolean;
}

/**
 * One fenced code block from a model reply.
 *
 * The code is rendered as a text child of `<pre><code>` — never as HTML — so a
 * reply that contains markup is displayed, not executed. Long lines scroll
 * horizontally rather than wrapping, which keeps columns lined up; tall blocks
 * scroll vertically so one answer cannot push the composer off screen.
 */
export function CodeBlock({ code, language = null, closed = true }: CodeBlockProps) {
  const [state, setState] = useState<CopyState>("idle");
  const timerRef = useRef<number | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  useEffect(() => clearTimer, [clearTimer]);

  const handleCopy = useCallback(() => {
    void copyText(code).then((copied) => {
      setState(copied ? "copied" : "failed");
      clearTimer();
      timerRef.current = window.setTimeout(() => {
        timerRef.current = null;
        setState("idle");
      }, FEEDBACK_MS);
    });
  }, [clearTimer, code]);

  const lineCount = code.split("\n").length;

  return (
    <figure className="codeblock">
      <figcaption className="codeblock__bar">
        <span className="codeblock__lang">
          {language ?? "code"}
          {closed ? null : <span className="codeblock__pending"> · streaming</span>}
        </span>

        <span className="codeblock__tools">
          <span className="codeblock__lines" aria-hidden="true">
            {lineCount} {lineCount === 1 ? "line" : "lines"}
          </span>

          <button
            type="button"
            className="codeblock__copy"
            onClick={handleCopy}
            aria-label={language ? `Copy ${language} code` : "Copy code"}
          >
            {state === "copied" ? "Copied" : state === "failed" ? "Copy failed" : "Copy"}
          </button>
        </span>
      </figcaption>

      {/* Focusable so a keyboard user can scroll a long block; the label only
          has an effect with a role, hence the explicit group. */}
      <pre
        className="codeblock__pre"
        tabIndex={0}
        role="group"
        aria-label={language ? `${language} code block` : "Code block"}
      >
        <code>{code}</code>
      </pre>

      <span className="sr-only" role="status" aria-live="polite">
        {state === "copied"
          ? "Code copied to the clipboard."
          : state === "failed"
            ? "The code could not be copied."
            : ""}
      </span>
    </figure>
  );
}
