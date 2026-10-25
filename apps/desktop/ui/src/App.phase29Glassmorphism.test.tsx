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

function contrastRatio(hexA: string, hexB: string): number {
  const lum = (hex: string) => {
    const r = parseInt(hex.slice(1, 3), 16) / 255;
    const g = parseInt(hex.slice(3, 5), 16) / 255;
    const b = parseInt(hex.slice(5, 7), 16) / 255;
    const f = (c: number) => (c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
    return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
  };
  const l1 = lum(hexA);
  const l2 = lum(hexB);
  return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
}

describe("Neutral Desktop Design System — Light & Dark Palette & Solid Surfaces", () => {
  const tokensCss = fs.readFileSync(path.resolve(__dirname, "styles/tokens.css"), "utf-8");

  it("defines solid opaque dark and light desktop themes with zero glass blur or aurora tint", () => {
    expect(tokensCss).toContain("--bg-base: #000000;");
    expect(tokensCss).not.toContain(':root[data-theme="light"]');
    expect(tokensCss).toContain("--glass-blur: none;");
    expect(tokensCss).toContain("--aurora-color-1: transparent;");
  });

  it("meets WCAG AA contrast ratios in both Dark (#111316 / #f3f4f6) and Light (#ffffff / #111827) themes", () => {
    expect(contrastRatio("#111316", "#f3f4f6")).toBeGreaterThan(12);
    expect(contrastRatio("#111316", "#b6bcc6")).toBeGreaterThan(8);
    expect(contrastRatio("#ffffff", "#111827")).toBeGreaterThan(14);
    expect(contrastRatio("#ffffff", "#4b5563")).toBeGreaterThan(7);
  });

  it("does not expose a theme toggle", async () => {
    render(<App />);
    expect(await screen.findByRole("main")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Switch to (light|dark) theme/i })).toBeNull();
  });
});
