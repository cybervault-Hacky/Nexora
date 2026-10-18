import { describe, it, expect } from "vitest";
import { confidenceLabel, edgeLabel } from "./analysis";

describe("analysis lib", () => {
  it("labels confidence", () => {
    expect(confidenceLabel("verified")).toBe("Verified");
    expect(confidenceLabel("inferred")).toBe("Inferred");
    expect(confidenceLabel("unresolved")).toBe("Inferred");
  });

  it("labels edge", () => {
    const edge = {
      from: "src/a.ts",
      to: "src/b.ts",
      raw: "./b",
      line: 3,
      verified: true,
      kind: "import",
      note: null,
    };
    expect(edgeLabel(edge as any)).toContain("src/a.ts");
    expect(edgeLabel(edge as any)).toContain("verified");
    const unresolved = { ...edge, verified: false, to: "./missing" };
    expect(edgeLabel(unresolved as any)).toContain("unresolved");
  });
});
