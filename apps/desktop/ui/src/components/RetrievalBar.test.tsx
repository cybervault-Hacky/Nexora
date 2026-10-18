import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RetrievalBar } from "./RetrievalBar";
import type { RetrievalResult } from "../features/workspace/retrieval";

describe("RetrievalBar", () => {
  it("renders nothing when null", () => {
    const { container } = render(<RetrievalBar retrieval={null} selectedPaths={new Set()} onAdd={vi.fn()} />);
    expect(container.innerHTML).toBe("");
  });

  it("shows suggestions and add button", async () => {
    const retrieval: RetrievalResult = {
      included: [
        { path: "src/app.ts", reason: "selected by you" },
        { path: "src/lib.ts", reason: "defines symbol `hello` (function)", symbol: { name: "hello", kind: "function", path: "src/lib.ts", startLine: 5, endLine: 5 } },
      ],
      omitted: [{ path: "src/large.ts", reason: "defines symbol `hello` — omitted (budget)" }],
      warnings: ["No relevant symbols found — using selected files only."],
      includedPaths: ["src/app.ts", "src/lib.ts"],
      omittedPaths: ["src/large.ts"],
    };
    const onAdd = vi.fn();
    const user = userEvent.setup();
    render(<RetrievalBar retrieval={retrieval} selectedPaths={new Set(["src/app.ts"])} onAdd={onAdd} />);
    expect(screen.getByText(/Retrieved for this prompt/i)).toBeTruthy();
    expect(screen.getByText(/1 selected file already in context/i)).toBeTruthy();
    expect(screen.getByText(/src\/lib.ts/i)).toBeTruthy();
    expect(screen.getAllByText(/defines symbol `hello`/i).length).toBeGreaterThanOrEqual(1);
    await user.click(screen.getByRole("button", { name: "Add src/lib.ts to context" }));
    expect(onAdd).toHaveBeenCalledWith("src/lib.ts");
    // omitted
    expect(screen.getByText(/Omitted 1/i)).toBeTruthy();
    expect(screen.getByText(/No relevant symbols/i)).toBeTruthy();
  });

  it("shows fallback when no suggestions", () => {
    const retrieval: RetrievalResult = {
      included: [{ path: "a.ts", reason: "selected by you" }],
      omitted: [],
      warnings: [],
      includedPaths: ["a.ts"],
      omittedPaths: [],
    };
    // All included are already selected, no warnings/omitted → should render nothing
    const { container } = render(<RetrievalBar retrieval={retrieval} selectedPaths={new Set(["a.ts"])} onAdd={vi.fn()} />);
    expect(container.innerHTML).toBe("");
  });

  it("shows note when no additional relevant files", () => {
    const retrieval: RetrievalResult = {
      included: [{ path: "a.ts", reason: "selected by you" }],
      omitted: [],
      warnings: ["No relevant symbols found — using selected files only."],
      includedPaths: ["a.ts"],
      omittedPaths: [],
    };
    render(<RetrievalBar retrieval={retrieval} selectedPaths={new Set(["a.ts"])} onAdd={vi.fn()} />);
    expect(screen.getByText(/No additional relevant files found/i)).toBeTruthy();
  });
});
