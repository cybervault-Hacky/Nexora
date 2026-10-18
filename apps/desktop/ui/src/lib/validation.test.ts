import { describe, it, expect } from "vitest";
import { overallLabel } from "./validation";

describe("validation helpers", () => {
  it("maps overall labels", () => {
    expect(overallLabel("passed")).toBe("Passed");
    expect(overallLabel("failed")).toBe("Failed");
    expect(overallLabel("notRun")).toBe("Not run");
    expect(overallLabel("unsupported")).toBe("Unsupported");
    expect(overallLabel("inconclusive")).toBe("Inconclusive");
  });
});
