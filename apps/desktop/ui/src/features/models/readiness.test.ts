import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import { READINESS_STATES } from "../../lib/ipc";
import type { ReadinessState } from "../../lib/ipc";
import { estimateLines, readinessLabel, readinessTone } from "./readiness";
import { makeReadiness } from "../../test/fixtures";

// Vitest may be started from the repo root, `apps/`, or `apps/desktop`, and
// jsdom gives `import.meta.url` an http scheme, so both layouts are tried.
const RUST_SOURCE = [
  resolve(process.cwd(), "src-tauri", "src", "inference", "readiness.rs"),
  resolve(process.cwd(), "desktop", "src-tauri", "src", "inference", "readiness.rs"),
  resolve(process.cwd(), "apps", "desktop", "src-tauri", "src", "inference", "readiness.rs"),
].find((candidate) => existsSync(candidate)) ?? "";

/**
 * The wire names the Rust core can actually emit, read from `as_str()`.
 *
 * This is a contract test, not a mirror of a constant someone typed twice: if
 * the backend adds or renames a state, this fails until the UI is updated.
 */
function rustStateNames(): string[] {
  const source = readFileSync(RUST_SOURCE, "utf8");
  const body = source.slice(
    source.indexOf("pub fn as_str(self)"),
    source.indexOf("impl fmt::Display for ReadinessState"),
  );

  return [...body.matchAll(/=>\s*"([a-z_]+)"/g)].map((match) => match[1] ?? "");
}

describe("readiness presentation", () => {
  it("matches every state name the Rust core emits", () => {
    const rust = rustStateNames();

    expect(rust).toHaveLength(14);
    expect([...READINESS_STATES].sort()).toEqual([...rust].sort());
  });

  it("gives every state a label and a tone", () => {
    for (const state of READINESS_STATES) {
      expect(readinessLabel(state)).not.toBe("");
      expect(readinessLabel(state)).not.toBe("Unknown state");
      expect(["neutral", "ok", "busy", "warning", "danger"]).toContain(readinessTone(state));
    }
  });

  it("labels the model file and the free-space requirement separately", () => {
    const lines = estimateLines(makeReadiness().estimate);

    expect(lines.fileGib).toBe("8.37");
    expect(lines.requiredFreeGib).toBe("8.87");
    expect(lines.memoryGib).toBe("10.9");
    expect(lines.context).toBe("8,192");
  });

  it("does not call an unknown state ready", () => {
    expect(readinessLabel("something_new" as ReadinessState)).toBe("Unknown state");
  });
});
