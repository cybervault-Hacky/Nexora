import { describe, it, expect } from "vitest";
import { languageForPath, isSupportedLanguage } from "./symbolIndex";

describe("symbolIndex helpers", () => {
  it("maps extensions to languages", () => {
    expect(languageForPath("src/main.rs")).toBe("rust");
    expect(languageForPath("a.ts")).toBe("typescript");
    expect(languageForPath("b.tsx")).toBe("tsx");
    expect(languageForPath("c.js")).toBe("javascript");
    expect(languageForPath("d.jsx")).toBe("jsx");
    expect(languageForPath("e.py")).toBe("python");
    expect(languageForPath("f.go")).toBe("unsupported");
    expect(languageForPath("noext")).toBe("unsupported");
  });

  it("reports supported languages", () => {
    expect(isSupportedLanguage("a.rs")).toBe(true);
    expect(isSupportedLanguage("a.ts")).toBe(true);
    expect(isSupportedLanguage("a.py")).toBe(true);
    expect(isSupportedLanguage("a.md")).toBe(false);
    expect(isSupportedLanguage("a.go")).toBe(false);
  });
});
