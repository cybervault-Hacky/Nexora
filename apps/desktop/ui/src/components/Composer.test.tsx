import { createRef } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Composer } from "./Composer";
import { MAX_MESSAGE_LENGTH, type ComposerError } from "../features/chat/chat";
import type { ImageReferenceMetadata } from "../features/chat/imageTo3dTask";

interface RenderOptions {
  value?: string;
  error?: ComposerError | null;
  busy?: boolean;
  canGenerate?: boolean;
  blockedReason?: string | null;
}

function renderOptions({
  value = "",
  error = null,
  busy = false,
  canGenerate = true,
  blockedReason = null,
}: RenderOptions = {}): string {
  return renderToStaticMarkup(
    <Composer
      value={value}
      error={error}
      textareaRef={createRef<HTMLTextAreaElement>()}
      busy={busy}
      canGenerate={canGenerate}
      blockedReason={blockedReason}
      onChange={() => undefined}
      onSubmit={() => undefined}
      onStop={() => undefined}
      onDismissError={() => undefined}
    />,
  );
}

function button(html: string, kind: "send" | "send send--stop"): string {
  const match = html.match(new RegExp(`<button[^>]*class="${kind}"[^>]*>`));

  if (!match) {
    throw new Error(`${kind} button not rendered`);
  }

  return match[0];
}

describe("Composer", () => {
  it("disables send for an empty draft", () => {
    expect(button(renderOptions(), "send")).toContain("disabled");
  });

  it("disables send for a whitespace-only draft", () => {
    expect(button(renderOptions({ value: "   \n\t" }), "send")).toContain("disabled");
  });

  it("enables send when there is text", () => {
    expect(button(renderOptions({ value: "hello" }), "send")).not.toContain("disabled");
  });

  it("disables send and explains itself when no model is loaded", () => {
    const html = renderOptions({ value: "hello", canGenerate: false });

    expect(button(html, "send")).toContain("disabled");
    expect(html).toContain("Load a model to send");
    expect(html).toContain("No model loaded");
  });

  it("explains the daily allowance instead of blaming a missing model", () => {
    const html = renderOptions({
      value: "hello",
      canGenerate: false,
      blockedReason: "Daily free allowance reached — it resets at 00:00 UTC",
    });

    expect(button(html, "send")).toContain("disabled");
    expect(html).toContain("Daily free allowance reached");
    expect(html).not.toContain("Load a model to send");
  });

  it("disables send past the length limit and shows the counter", () => {
    const html = renderOptions({ value: "a".repeat(MAX_MESSAGE_LENGTH + 1) });

    expect(button(html, "send")).toContain("disabled");
    expect(html).toContain("composer__counter--danger");
  });

  it("hides the counter for short messages", () => {
    expect(renderOptions({ value: "hello" })).not.toContain("composer__counter");
  });

  it("surfaces an error accessibly", () => {
    const html = renderOptions({
      error: { code: "empty", message: "Enter a message before sending." },
    });

    expect(html).toContain('role="alert"');
    expect(html).toContain("Enter a message before sending.");
    expect(html).toContain('aria-invalid="true"');
    expect(html).toContain("composer--error");
  });

  it("replaces send with a stop control while streaming", () => {
    const html = renderOptions({ value: "", busy: true });

    expect(html).toContain('aria-label="Stop generating"');
    expect(html).toContain("Generating on this machine");
    expect(html).not.toContain('aria-label="Send message"');
  });

  it("never renders a fabricated model message", () => {
    const html = renderOptions({ value: "" });

    expect(html).not.toContain("Scaffold");
    expect(html).not.toContain("demo reply");
  });

  it("has no attachment, microphone, or model-switcher controls", () => {
    const html = renderOptions({ value: "hello" });

    expect(html).not.toContain("attach");
    expect(html).not.toContain("microphone");
    expect(html).not.toContain("model-picker");
  });

});

describe("Composer interaction", () => {
  function setup({ busy = false, canGenerate = true, value = "hello" } = {}) {
    const onSubmit = vi.fn();
    const onStop = vi.fn();
    const onChange = vi.fn();

    render(
      <Composer
        value={value}
        error={null}
        textareaRef={createRef<HTMLTextAreaElement>()}
        busy={busy}
        canGenerate={canGenerate}
        onChange={onChange}
        onSubmit={onSubmit}
        onStop={onStop}
        onDismissError={() => undefined}
      />,
    );

    return { onSubmit, onStop, onChange, user: userEvent.setup() };
  }

  it("submits with Enter and not with Shift+Enter", async () => {
    const { onSubmit, user } = setup();

    await user.type(screen.getByLabelText("Message Nexora"), "{Enter}");
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("textbox")).toBeTruthy();

    await user.keyboard("{Shift>}{Enter}{/Shift}");
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it("sends from the send button", async () => {
    const { onSubmit, user } = setup();

    await user.click(screen.getByRole("button", { name: "Send message" }));
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it("stops with Escape while generating", async () => {
    const { onStop, user } = setup({ busy: true, value: "" });

    await user.type(screen.getByLabelText("Message Nexora"), "{Escape}");
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it("stops from the stop button", async () => {
    const { onStop, user } = setup({ busy: true, value: "" });

    await user.click(screen.getByRole("button", { name: "Stop generating" }));
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it("cannot send while the model is unavailable", async () => {
    const { onSubmit, user } = setup({ canGenerate: false });

    await user.click(screen.getByRole("button", { name: "Send message" }));
    await user.type(screen.getByLabelText("Message Nexora"), "{Enter}");

    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("reports every keystroke to the owner", async () => {
    const { onChange, user } = setup({ value: "" });

    await user.type(screen.getByLabelText("Message Nexora"), "ab");
    expect(onChange).toHaveBeenCalled();
  });

  it("lets the user choose a PNG reference without requiring a chat model", async () => {
    const onPickImageReference = vi.fn();
    const user = userEvent.setup();
    render(
      <Composer
        value="Make a stone arch"
        error={null}
        textareaRef={createRef<HTMLTextAreaElement>()}
        busy={false}
        canGenerate={false}
        allowSubmitWithoutModel
        onPickImageReference={onPickImageReference}
        onChange={() => undefined}
        onSubmit={() => undefined}
        onStop={() => undefined}
        onDismissError={() => undefined}
      />,
    );
    await user.upload(screen.getByLabelText("Choose PNG image reference"), new File(["png"], "reference.png", { type: "image/png" }));
    expect(onPickImageReference).toHaveBeenCalledTimes(1);
    expect((screen.getByRole("button", { name: "Send message" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("shows preview metadata and supports removal for a validated reference", async () => {
    const reference: ImageReferenceMetadata = {
      schemaVersion: 1,
      referenceId: "0123456789abcdef0123456789abcdef",
      format: "image/png",
      width: 512,
      height: 512,
      sizeBytes: 128_000,
      processingState: "validated",
      validationErrors: [],
    };
    const onRemoveImageReference = vi.fn();
    const user = userEvent.setup();
    render(
      <Composer
        value="Create a 3D arch"
        error={null}
        textareaRef={createRef<HTMLTextAreaElement>()}
        busy={false}
        canGenerate={false}
        allowSubmitWithoutModel
        imageReference={{ metadata: reference, previewUrl: "blob:test-preview" }}
        onPickImageReference={vi.fn()}
        onRemoveImageReference={onRemoveImageReference}
        onChange={() => undefined}
        onSubmit={() => undefined}
        onStop={() => undefined}
        onDismissError={() => undefined}
      />,
    );
    expect(screen.getByAltText("Selected image reference preview")).toBeTruthy();
    expect(screen.getAllByText(/Image attached · Reference validated/)).toHaveLength(2);
    expect(screen.getAllByText(/Analysis unavailable/)).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Replace image" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Remove image reference" }));
    expect(onRemoveImageReference).toHaveBeenCalledTimes(1);
  });
});
