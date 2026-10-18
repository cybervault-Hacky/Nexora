import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CodeBlock } from "./CodeBlock";

/** The clipboard is not implemented in jsdom; each test installs its own. */
function installClipboard(writeText: (text: string) => Promise<void>) {
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText },
    configurable: true,
  });
}

beforeEach(() => {
  installClipboard(vi.fn().mockResolvedValue(undefined));
});

afterEach(() => {
  Reflect.deleteProperty(navigator, "clipboard");
  vi.useRealTimers();
});

describe("CodeBlock", () => {
  it("shows the language label and the code verbatim", () => {
    render(<CodeBlock language="typescript" code={"const a = 1;\n  const b = 2;"} />);

    expect(screen.getByText("typescript")).toBeDefined();
    expect(screen.getByText(/const a = 1;/)).toBeDefined();
    // Indentation must survive: it is what makes code readable.
    expect(screen.getByRole("code").textContent).toBe(
      "const a = 1;\n  const b = 2;",
    );
  });

  it("falls back to a neutral label and marks a streamed block", () => {
    render(<CodeBlock code="partial" closed={false} />);

    expect(screen.getByText(/code/)).toBeDefined();
    expect(screen.getByText(/streaming/)).toBeDefined();
    expect(screen.queryByText(/streaming/)).not.toBeNull();
  });

  it("does not mark a finished block as streaming", () => {
    render(<CodeBlock code="done" language="sh" />);

    expect(screen.queryByText(/streaming/)).toBeNull();
  });

  it("reports the line count", () => {
    render(<CodeBlock code={"a\nb\nc"} />);

    expect(screen.getByText("3 lines")).toBeDefined();
  });

  it("copies the exact code and confirms it", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    installClipboard(writeText);

    render(<CodeBlock language="rust" code="fn main() {}" />);

    await userEvent.click(screen.getByRole("button", { name: "Copy rust code" }));

    expect(writeText).toHaveBeenCalledWith("fn main() {}");
    expect(await screen.findByText("Copied")).toBeDefined();
  });

  it("says so when the copy fails, rather than claiming success", async () => {
    installClipboard(vi.fn().mockRejectedValue(new Error("denied")));

    render(<CodeBlock code="x" />);

    await userEvent.click(screen.getByRole("button", { name: "Copy code" }));

    expect(await screen.findByText("Copy failed")).toBeDefined();
    expect(screen.queryByText("Copied")).toBeNull();
  });

  it("returns the button to its resting state", async () => {
    vi.useFakeTimers();

    render(<CodeBlock code="x" />);

    await act(async () => {
      screen.getByRole("button", { name: "Copy code" }).click();
      await Promise.resolve();
    });

    expect(screen.getByText("Copied")).toBeDefined();

    act(() => {
      vi.advanceTimersByTime(2_000);
    });

    expect(screen.queryByText("Copied")).toBeNull();
    expect(screen.getByRole("button", { name: "Copy code" })).toBeDefined();
  });

  it("shows untrusted markup as text instead of creating elements", () => {
    const payload = '<img src=x onerror="alert(1)"><script>alert(2)</script>';

    const { container } = render(<CodeBlock language="html" code={payload} />);

    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByRole("code").textContent).toBe(payload);
  });

  it("keeps a very long line intact inside a scrollable region", () => {
    const long = "x".repeat(5_000);

    const { container } = render(<CodeBlock code={long} language="txt" />);

    expect(screen.getByRole("code").textContent).toBe(long);
    expect(container.querySelector(".codeblock__pre")).not.toBeNull();
  });

  it("announces the copy result for assistive technology", async () => {
    render(<CodeBlock code="x" />);

    await userEvent.click(screen.getByRole("button", { name: "Copy code" }));

    const status = await screen.findByRole("status");

    expect(status.textContent).toBe("Code copied to the clipboard.");
  });
});
