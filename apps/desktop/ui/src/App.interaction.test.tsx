import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import App from "./App";

describe("Nexora interactions", () => {
  it("sends a message with Enter and shows the labelled scaffold reply", async () => {
    const user = userEvent.setup();
    render(<App />);

    const input = screen.getByLabelText("Message Nexora");
    await user.type(input, "build a pricing page{Enter}");

    expect(screen.getByText("build a pricing page")).toBeTruthy();
    expect(screen.getByText("Scaffold · no model")).toBeTruthy();
    expect(screen.getByRole("log")).toBeTruthy();
    expect((input as HTMLTextAreaElement).value).toBe("");
  });

  it("keeps newlines with Shift+Enter and does not submit", async () => {
    const user = userEvent.setup();
    render(<App />);

    const input = screen.getByLabelText("Message Nexora");
    await user.type(input, "line one{Shift>}{Enter}{/Shift}line two");

    expect((input as HTMLTextAreaElement).value).toBe("line one\nline two");
    expect(screen.queryByText("Scaffold · no model")).toBeNull();
  });

  it("does not send an empty or whitespace-only message", async () => {
    const user = userEvent.setup();
    render(<App />);

    const input = screen.getByLabelText("Message Nexora");
    await user.type(input, "   {Enter}");

    expect(screen.queryByText("Scaffold · no model")).toBeNull();
    expect(screen.getByRole("alert").textContent).toContain(
      "Enter a message before sending.",
    );
  });

  it("clears the conversation when a new chat starts", async () => {
    const user = userEvent.setup();
    render(<App />);

    const input = screen.getByLabelText("Message Nexora");
    await user.type(input, "first message{Enter}");
    expect(screen.getByText("first message")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "New chat" }));

    expect(screen.queryByText("first message")).toBeNull();
    expect(screen.queryByText("Scaffold · no model")).toBeNull();
    expect((input as HTMLTextAreaElement).value).toBe("");
    expect(screen.getByText("Build something real.")).toBeTruthy();
  });

  it("collapses and expands the sidebar from the title bar", async () => {
    const user = userEvent.setup();
    render(<App />);

    const toggle = screen.getByRole("button", { name: "Collapse sidebar" });
    expect(toggle.getAttribute("aria-expanded")).toBe("true");

    await user.click(toggle);

    const expandedToggle = screen.getByRole("button", { name: "Expand sidebar" });
    expect(expandedToggle.getAttribute("aria-expanded")).toBe("false");

    await user.click(expandedToggle);
    expect(
      screen.getByRole("button", { name: "Collapse sidebar" }).getAttribute(
        "aria-expanded",
      ),
    ).toBe("true");
  });

  it("shows an honest engine state and no model controls", () => {
    render(<App />);

    const sidebar = screen.getByLabelText("Workspace");
    expect(within(sidebar).getByText("Not connected")).toBeTruthy();
    expect(within(sidebar).queryByText("Connected")).toBeNull();

    expect(screen.queryByLabelText(/attach/i)).toBeNull();
    expect(screen.queryByLabelText(/microphone/i)).toBeNull();
    expect(screen.queryByLabelText(/choose model/i)).toBeNull();
  });
});
