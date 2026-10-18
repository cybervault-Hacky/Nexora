import { invoke } from "@tauri-apps/api/core";
import { markDesktop } from "../test/fixtures";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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
    expect(loaded.messages[0]!.state).toBe("stopped");
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

describe("browser storage integrity", () => {
  beforeEach(() => { markDesktop(false); clearFallbackStorage(); });
  it("does not replace corrupt records with empty history", async () => {
    const {listConversations,createConversation} = await import("./history");
    localStorage.setItem("nexora:conversations:v1", "not-json");
    await expect(listConversations()).rejects.toMatchObject({code:"storage"});
    await expect(createConversation("No overwrite")).rejects.toMatchObject({code:"storage"});
    expect(localStorage.getItem("nexora:conversations:v1")).toBe("not-json");
    clearFallbackStorage();
  });
  it("reports quota failures instead of claiming a save", async () => {
    const {createConversation} = await import("./history");
    const spy = vi.spyOn(Storage.prototype,"setItem").mockImplementation(() => { throw new Error("Quota"); });
    await expect(createConversation("Not saved")).rejects.toMatchObject({code:"storage"});
    spy.mockRestore();
  });
});

describe("desktop storage boundary", () => {
  beforeEach(() => { markDesktop(); vi.mocked(invoke).mockReset(); });
  afterEach(() => markDesktop(false));
  it("propagates save and load errors without a localStorage fallback", async () => {
    const { listConversations, saveConversation } = await import("./history");
    vi.mocked(invoke).mockRejectedValue({code:"storage", message:"Disk full"});
    await expect(listConversations()).rejects.toMatchObject({code:"storage"});
    await expect(saveConversation({id:"one",title:"One",createdAt:1,updatedAt:1,messages:[]}))
      .rejects.toMatchObject({message:"Disk full"});
  });
  it("orders saves before a later read or deletion", async () => {
    const { saveConversation, loadConversation, deleteConversation } = await import("./history");
    let release!: (value: unknown) => void;
    vi.mocked(invoke).mockImplementationOnce(() => new Promise(resolve => { release = resolve; }));
    vi.mocked(invoke).mockResolvedValueOnce({id:"one"}).mockResolvedValueOnce(true);
    const save = saveConversation({id:"one",title:"One",createdAt:1,updatedAt:1,messages:[]});
    const load = loadConversation("one"); const del = deleteConversation("one");
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    release({id:"one"}); await save; await load; await del;
    expect(vi.mocked(invoke).mock.calls.map(c => c[0])).toEqual(["save_conversation","load_conversation","delete_conversation"]);
  });
});
