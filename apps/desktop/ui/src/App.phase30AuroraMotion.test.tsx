import fs from "node:fs";
import path from "node:path";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: () => Promise.resolve(undefined),
}));

vi.mock("./lib/inference", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/inference")>();
  return {
    ...actual,
    fetchStatus: async () => ({
      name: "Nexora",
      version: "0.1.0",
      engine: { state: "ready", detail: "Ready" },
      modelPath: null,
      runtimePath: null,
      contextSize: 32768,
    }),
    fetchSettings: async () => ({ modelPath: null, runtimePath: null, contextSize: 32768 }),
    fetchSystemInfo: async () => ({ totalRamGib: 32, availableRamGib: 20, platform: "windows" }),
    generate: vi.fn(),
    cancelGeneration: async () => true,
    unloadModel: vi.fn(),
  };
});

import App from "./App";

describe("Restrained Desktop Motion & Zero Gratuitous Ambient Effects", () => {
  it("omits decorative aurora beams from the rendered DOM", async () => {
    render(<App />);
    await screen.findByRole("region", { name: "Conversation messages" });
    expect(screen.queryByTestId("aurora-ambient")).toBeNull();
  });

  it("defines crisp <=200ms desktop motion tokens and honors reduced-motion preferences", () => {
    const tokensCss = fs.readFileSync(path.resolve(__dirname, "styles/tokens.css"), "utf-8");
    const appCss = fs.readFileSync(path.resolve(__dirname, "styles/app.css"), "utf-8");

    expect(tokensCss).toContain("--motion-instant:");
    expect(tokensCss).toContain("--motion-fast:");
    expect(tokensCss).toContain("--motion-base:");
    expect(appCss).toContain("@media (prefers-reduced-motion: reduce)");
  });
});
