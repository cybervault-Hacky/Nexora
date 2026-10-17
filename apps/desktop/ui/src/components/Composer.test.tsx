import { createRef } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { Composer } from "./Composer";
import { MAX_MESSAGE_LENGTH, type ComposerError } from "../features/chat/chat";

function render(value: string, error: ComposerError | null = null): string {
  return renderToStaticMarkup(
    <Composer
      value={value}
      error={error}
      textareaRef={createRef<HTMLTextAreaElement>()}
      onChange={() => undefined}
      onSubmit={() => undefined}
      onDismissError={() => undefined}
    />,
  );
}

function sendButton(html: string): string {
  const match = html.match(/<button[^>]*class="send"[^>]*>/);

  if (!match) {
    throw new Error("send button not rendered");
  }

  return match[0];
}

describe("Composer", () => {
  it("disables send for an empty draft", () => {
    expect(sendButton(render(""))).toContain("disabled");
  });

  it("disables send for a whitespace-only draft", () => {
    expect(sendButton(render("   \n\t"))).toContain("disabled");
  });

  it("enables send when there is text", () => {
    expect(sendButton(render("hello"))).not.toContain("disabled");
  });

  it("disables send past the length limit and shows the counter", () => {
    const html = render("a".repeat(MAX_MESSAGE_LENGTH + 1));

    expect(sendButton(html)).toContain("disabled");
    expect(html).toContain("composer__counter--danger");
  });

  it("hides the counter for short messages", () => {
    expect(render("hello")).not.toContain("composer__counter");
  });

  it("surfaces an error accessibly", () => {
    const html = render("", { code: "empty", message: "Enter a message before sending." });

    expect(html).toContain('role="alert"');
    expect(html).toContain("Enter a message before sending.");
    expect(html).toContain('aria-invalid="true"');
    expect(html).toContain("composer--error");
  });

  it("has no attachment, microphone, or model-switcher controls", () => {
    const html = render("hello");

    expect(html).not.toContain("attach");
    expect(html).not.toContain("microphone");
    expect(html).not.toContain("model-picker");
  });
});
