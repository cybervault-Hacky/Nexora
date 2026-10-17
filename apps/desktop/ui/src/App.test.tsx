import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import App from "./App";

const html = renderToStaticMarkup(<App />);

describe("Nexora shell", () => {
  it("renders the product identity and empty state", () => {
    expect(html).toContain("Nexora");
    expect(html).toContain("Build something real.");
    expect(html).toContain("Local-first AI coding");
  });

  it("states that the engine is not connected", () => {
    expect(html).toContain("Engine not connected");
    expect(html).toContain("No model connected");
  });

  it("labels the composer and its input accessibly", () => {
    expect(html).toContain("Message Nexora");
    expect(html).toContain("Describe what you want to build");
    expect(html).toContain('aria-label="Send message"');
  });

  it("offers a sidebar new-chat action and collapse control", () => {
    expect(html).toContain('aria-label="New chat"');
    expect(html).toContain('aria-controls="sidebar"');
    expect(html).toContain('aria-expanded="true"');
  });

  it("does not claim that inference is working", () => {
    expect(html).not.toContain("Engine ready");
    expect(html).not.toContain("AI is ready");
    expect(html).not.toContain("Connected</span>");
  });

  it("keeps the empty state quiet for screen readers", () => {
    // The live log only exists once a conversation has started, so the empty
    // state never announces anything.
    expect(html).not.toContain('role="log"');
    expect(html).toContain("Example prompts");
  });
});
