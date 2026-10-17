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
    expect(html).toContain("No model loaded");
  });

  it("renders the composer and its accessible label", () => {
    expect(html).toContain("Message Nexora");
    expect(html).toContain("Describe what you want to build");
  });

  it("does not claim that inference is working", () => {
    expect(html).not.toContain("Engine ready");
    expect(html).not.toContain("AI is ready");
  });
});
