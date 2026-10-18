import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { HistoryList } from "./HistoryList";

const conv = { id: "one", title: "A chat", createdAt: 1, updatedAt: 2, messageCount: 2, preview: "Hello" };
function props() {
  return { conversations: [conv], activeId: "one", loading: false, error: null,
    collapsed: false, onSelect: vi.fn(), onRename: vi.fn().mockResolvedValue(undefined),
    onDelete: vi.fn().mockResolvedValue(undefined), onRetry: vi.fn() };
}
describe("history switcher", () => {
  it("does not repeat message count when a conversation has no preview", () => {
    const p = props();
    const { container } = render(<HistoryList {...p} conversations={[{ ...conv, preview: "" }]} />);
    expect(container.querySelector(".history__preview")).toBeNull();
    expect(screen.getByText(/2 msg/)).toBeTruthy();
  });

  it("selects by keyboard with a current state", async () => {
    const p = props(); render(<HistoryList {...p}/>);
    const item = screen.getByRole("button", {name: "A chat Hello"});
    expect(item.getAttribute("aria-current")).toBe("true");
    item.focus(); await userEvent.keyboard("{Enter}");
    expect(p.onSelect).toHaveBeenCalledWith("one");
  });
  it("requires explicit delete confirmation and supports cancelling", async () => {
    const p = props(); render(<HistoryList {...p}/>);
    await userEvent.click(screen.getByRole("button", {name: "Delete A chat"}));
    expect(p.onDelete).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", {name: "Cancel delete"}));
    expect(p.onDelete).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", {name: "Delete A chat"}));
    await userEvent.click(screen.getByRole("button", {name: "Confirm delete A chat"}));
    expect(p.onDelete).toHaveBeenCalledWith("one");
    expect(p.onSelect).not.toHaveBeenCalled();
  });
  it("renames without selecting or starting inference", async () => {
    const p = props(); render(<HistoryList {...p}/>);
    await userEvent.click(screen.getByRole("button", {name: "Rename A chat"}));
    const input = screen.getByRole("textbox", {name: "Edit conversation title"});
    await userEvent.clear(input); await userEvent.type(input,"New name{Enter}");
    expect(p.onRename).toHaveBeenCalledWith("one", "New name");
    expect(p.onSelect).not.toHaveBeenCalled();
  });
  it("keeps confirmation on delete failure", async () => {
    const p = props(); p.onDelete.mockRejectedValue(new Error("Disk full")); render(<HistoryList {...p}/>);
    await userEvent.click(screen.getByRole("button", {name: "Delete A chat"}));
    await userEvent.click(screen.getByRole("button", {name: "Confirm delete A chat"}));
    expect(screen.getByRole("button", {name: "Cancel delete"})).toBeTruthy();
  });
  it("shows loading, empty, and retryable errors", async () => {
    const p = props(); const {rerender} = render(<HistoryList {...p} loading/>);
    expect(screen.getByRole("status").textContent).toContain("Loading");
    rerender(<HistoryList {...p} conversations={[]}/>);
    expect(screen.getByText(/No conversations yet/)).toBeTruthy();
    rerender(<HistoryList {...p} error="History damaged"/>);
    expect(screen.getByRole("alert").textContent).toContain("History damaged");
    await userEvent.click(screen.getByRole("button", {name: "Retry"}));
    expect(p.onRetry).toHaveBeenCalledOnce();
  });
});
