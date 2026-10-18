import { beforeEach, describe, expect, it } from "vitest";
import { clearFallbackStorage } from "./history";

describe("history lib fallback", () => {
  beforeEach(() => {
    clearFallbackStorage();
    // Ensure we are in browser mode (no Tauri)
    // @ts-ignore delete
    delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it("exports required types via runtime", async () => {
    const mod = await import("./history");
    expect(typeof mod.listConversations).toBe("function");
    expect(typeof mod.loadConversation).toBe("function");
    expect(typeof mod.createConversation).toBe("function");
    expect(typeof mod.saveConversation).toBe("function");
    expect(typeof mod.renameConversation).toBe("function");
    expect(typeof mod.deleteConversation).toBe("function");
  });

  it("creates and lists conversations via fallback", async () => {
    const { createConversation, listConversations } = await import("./history");
    const conv = await createConversation("Hello");
    expect(conv.title).toBe("Hello");
    expect(conv.messages).toHaveLength(0);

    const list = await listConversations();
    expect(list.length).toBe(1);
    expect(list[0]!.title).toBe("Hello");
  });

  it("saves and loads with preview", async () => {
    const { createConversation, saveConversation, loadConversation } = await import("./history");
    const conv = await createConversation("Test");
    const updated = await saveConversation({
      ...conv,
      messages: [
        { id: "m1", role: "user", content: "Hello world", createdAt: Date.now() },
        { id: "m2", role: "assistant", content: "Hi there\nsecond line", createdAt: Date.now(), state: "complete" },
      ],
    });
    expect(updated.messages.length).toBe(2);

    const loaded = await loadConversation(conv.id);
    expect(loaded.messages.length).toBe(2);
    expect(loaded.messages[0]!.content).toBe("Hello world");
  });

  it("renames and deletes", async () => {
    const { createConversation, renameConversation, deleteConversation, listConversations } = await import("./history");
    const conv = await createConversation("Old");
    const renamed = await renameConversation(conv.id, "New");
    expect(renamed.title).toBe("New");

    await deleteConversation(conv.id);
    const list = await listConversations();
    expect(list.length).toBe(0);
  });

  it("handles not_found for missing id", async () => {
    const { loadConversation } = await import("./history");
    await expect(loadConversation("nonexistent-id")).rejects.toMatchObject({ code: "not_found" });
  });

  it("normalizes streaming state on save", async () => {
    const { createConversation, saveConversation, loadConversation } = await import("./history");
    const conv = await createConversation("Stream");
    await saveConversation({
      ...conv,
      messages: [
        { id: "m1", role: "assistant", content: "partial", createdAt: Date.now(), state: "streaming" },
        { id: "m2", role: "assistant", content: "", createdAt: Date.now(), state: "streaming" },
      ],
    });
    const loaded = await loadConversation(conv.id);
    expect(loaded.messages[0]!.state).toBe("complete");
    expect(loaded.messages[1]!.state).toBe("stopped");
  });

  it("clears fallback storage", async () => {
    const { createConversation, listConversations } = await import("./history");
    await createConversation("A");
    clearFallbackStorage();
    const list = await listConversations();
    expect(list.length).toBe(0);
  });
});
