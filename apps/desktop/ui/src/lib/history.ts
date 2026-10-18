import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell } from "./ipc";

export interface HistoryMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  createdAt: number;
  state?: string;
  failure?: string;
}

export interface PersistedConversation {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  messages: HistoryMessage[];
}

export interface ConversationSummary {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  messageCount: number;
  preview: string;
}

export interface HistoryError {
  code: string;
  message: string;
}

function toHistoryError(cause: unknown): HistoryError {
  if (
    typeof cause === "object" &&
    cause !== null &&
    "code" in cause &&
    "message" in cause &&
    typeof (cause as HistoryError).code === "string" &&
    typeof (cause as HistoryError).message === "string"
  ) {
    return cause as HistoryError;
  }
  if (typeof cause === "string" && cause.trim().length > 0) {
    return { code: "ipc", message: cause };
  }
  if (cause instanceof Error && cause.message.trim().length > 0) {
    return { code: "ipc", message: cause.message };
  }
  return { code: "ipc", message: "The desktop core did not answer." };
}

// ---------------------------------------------------------------------------
// Browser fallback (localStorage) — used in browser preview and jsdom tests
// ---------------------------------------------------------------------------

const STORAGE_KEY = "nexora:conversations:v1";

interface FallbackFile {
  version: number;
  conversations: PersistedConversation[];
}

function validateBrowserFile(file: FallbackFile): void {
  const timestamp = (v: unknown) => typeof v === "number" && Number.isSafeInteger(v) && v >= 0 && v <= 8_640_000_000_000_000;
  const ids = new Set<string>();
  if (file.version !== 1 || file.conversations.length > 500) throw new Error("Invalid history schema");
  for (const c of file.conversations) {
    if (!c || typeof c.id !== "string" || !isValidId(c.id) || ids.has(c.id) ||
      typeof c.title !== "string" || Array.from(c.title).length > 200 ||
      !timestamp(c.createdAt) || !timestamp(c.updatedAt) || c.updatedAt < c.createdAt ||
      !Array.isArray(c.messages) || c.messages.length > 2000) throw new Error("Invalid conversation");
    ids.add(c.id);
    const messageIds = new Set<string>();
    for (const m of c.messages) {
      if (!m || typeof m.id !== "string" || !isValidId(m.id) || messageIds.has(m.id) ||
        !["user","assistant"].includes(m.role) || typeof m.content !== "string" ||
        Array.from(m.content).length > 400_000 || !timestamp(m.createdAt) ||
        (m.state !== undefined && !["complete","stopped","streaming","error"].includes(m.state)) ||
        (m.failure !== undefined && (typeof m.failure !== "string" || Array.from(m.failure).length > 2000)))
        throw new Error("Invalid message");
      messageIds.add(m.id);
    }
  }
}

function fallbackLoad(): FallbackFile {
  try {
    if (typeof localStorage === "undefined") return { version: 1, conversations: [] };
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { version: 1, conversations: [] };
    const parsed = JSON.parse(raw) as FallbackFile;
    if (!parsed || typeof parsed.version !== "number" || !Array.isArray(parsed.conversations)) {
      throw new Error("Malformed history");
    }
    if (parsed.version !== 1) throw new Error("Unsupported history version");
    validateBrowserFile(parsed);
    return parsed;
  } catch {
    throw { code: "storage", message: "Browser history is unreadable. Original data preserved." };
  }
}

function fallbackSave(file: FallbackFile): void {
  try {
    if (typeof localStorage === "undefined") return;
    validateBrowserFile(file);
    const text = JSON.stringify(file);
    if (new TextEncoder().encode(text).length > 10 * 1024 * 1024) throw new Error("History too large");
    localStorage.setItem(STORAGE_KEY, text);
  } catch {
    throw { code: "storage", message: "Could not save browser history. Check storage permissions and quota." };
  }
}

function isValidId(id: string): boolean {
  return id.length > 0 && id.length <= 128 && /^[A-Za-z0-9_-]+$/.test(id);
}

function sanitizeTitle(title: string): string {
  const t = title.trim();
  if (t.length === 0) return "New conversation";
  const cleaned = t.replace(/[\r\n]+/g, " ").replace(/\s+/g, " ").trim();
  if (cleaned.length === 0) return "New conversation";
  return cleaned.slice(0, 200);
}

function generateFallbackId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `id-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

// ---------------------------------------------------------------------------
// Desktop (Tauri) paths
// ---------------------------------------------------------------------------

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toHistoryError(cause);
  }
}

async function listConversationsImpl(): Promise<ConversationSummary[]> {
  if (!isDesktopShell()) {
    const file = fallbackLoad();
    const summaries: ConversationSummary[] = file.conversations
      .filter((c) => isValidId(c.id))
      .map((c) => ({
        id: c.id,
        title: sanitizeTitle(c.title),
        createdAt: c.createdAt,
        updatedAt: c.updatedAt,
        messageCount: c.messages.length,
        preview: c.messages.length > 0 ? c.messages[c.messages.length - 1]!.content.slice(0, 120) : "",
      }));
    summaries.sort((a, b) => b.updatedAt - a.updatedAt);
    return summaries;
  }
  return call<ConversationSummary[]>("list_conversations");
}

async function loadConversationImpl(id: string): Promise<PersistedConversation> {
  if (!isDesktopShell()) {
    const file = fallbackLoad();
    const conv = file.conversations.find((c) => c.id === id);
    if (!conv) throw toHistoryError({ code: "not_found", message: `Conversation ${id} not found` });
    const messages = conv.messages.map((m) => {
      if (m.state === "streaming") {
        return { ...m, state: "stopped" };
      }
      return m;
    });
    return { ...conv, messages };
  }
  return call<PersistedConversation>("load_conversation", { id });
}

async function createConversationImpl(title?: string): Promise<PersistedConversation> {
  if (!isDesktopShell()) {
    const file = fallbackLoad();
    const now = Date.now();
    const conv: PersistedConversation = {
      id: generateFallbackId(),
      title: sanitizeTitle(title ?? "New conversation"),
      createdAt: now,
      updatedAt: now,
      messages: [],
    };
    file.conversations.push(conv);
    fallbackSave(file);
    return conv;
  }
  return call<PersistedConversation>("create_conversation", { title: title ?? null });
}

async function createConversationWithIdImpl(id: string, title?: string): Promise<PersistedConversation> {
  if (!isDesktopShell()) {
    const file = fallbackLoad();
    if (file.conversations.some((c) => c.id === id)) {
      throw toHistoryError({ code: "invalid_request", message: "Conversation id already exists" });
    }
    const now = Date.now();
    const conv: PersistedConversation = {
      id,
      title: sanitizeTitle(title ?? "New conversation"),
      createdAt: now,
      updatedAt: now,
      messages: [],
    };
    file.conversations.push(conv);
    fallbackSave(file);
    return conv;
  }
  return call<PersistedConversation>("create_conversation", { id, title: title ?? null });
}

async function saveConversationImpl(conversation: PersistedConversation): Promise<PersistedConversation> {
  // Normalize streaming state before save, mirroring Rust logic
  const normalized: PersistedConversation = {
    ...conversation,
    messages: conversation.messages.map((m) => {
      if (m.state === "streaming") {
        return { ...m, state: "stopped" };
      }
      return m;
    }),
  };

  if (!isDesktopShell()) {
    const file = fallbackLoad();
    const idx = file.conversations.findIndex((c) => c.id === normalized.id);
    const toSave = { ...normalized, title: sanitizeTitle(normalized.title), updatedAt: Date.now() };
    if (idx >= 0) {
      file.conversations[idx] = toSave;
    } else {
      file.conversations.push(toSave);
    }
    fallbackSave(file);
    return toSave;
  }
  return call<PersistedConversation>("save_conversation", { conversation: normalized });
}

async function renameConversationImpl(id: string, title: string): Promise<ConversationSummary> {
  if (!isDesktopShell()) {
    const file = fallbackLoad();
    const conv = file.conversations.find((c) => c.id === id);
    if (!conv) throw toHistoryError({ code: "not_found", message: `Conversation ${id} not found` });
    conv.title = sanitizeTitle(title);
    conv.updatedAt = Date.now();
    fallbackSave(file);
    return {
      id: conv.id,
      title: conv.title,
      createdAt: conv.createdAt,
      updatedAt: conv.updatedAt,
      messageCount: conv.messages.length,
      preview: conv.messages.length > 0 ? conv.messages[conv.messages.length - 1]!.content.slice(0, 120) : "",
    };
  }
  return call<ConversationSummary>("rename_conversation", { id, title });
}

async function deleteConversationImpl(id: string): Promise<boolean> {
  if (!isDesktopShell()) {
    const file = fallbackLoad();
    const before = file.conversations.length;
    file.conversations = file.conversations.filter((c) => c.id !== id);
    if (file.conversations.length === before) {
      throw toHistoryError({ code: "not_found", message: `Conversation ${id} not found` });
    }
    fallbackSave(file);
    return true;
  }
  return call<boolean>("delete_conversation", { id });
}

// Utility for tests: clear fallback storage
export function clearFallbackStorage(): void {
  try {
    if (typeof localStorage !== "undefined") localStorage.removeItem(STORAGE_KEY);
  } catch {
    // ignore
  }
}

// One ordered IPC lane: an older save can never complete after a newer save,
// rename, delete, or read issued by this webview. Rejections do not poison it.
let lane: Promise<unknown> = Promise.resolve();
function ordered<T>(action: () => Promise<T>): Promise<T> {
  const next = lane.then(action);
  lane = next.catch(() => undefined);
  return next;
}
export function listConversations() { return ordered(() => listConversationsImpl()); }
export function loadConversation(id: string) { return ordered(() => loadConversationImpl(id)); }
export function createConversation(title?: string) { return ordered(() => createConversationImpl(title)); }
export function createConversationWithId(id: string, title?: string) { return ordered(() => createConversationWithIdImpl(id, title)); }
export function saveConversation(conversation: PersistedConversation) { return ordered(() => saveConversationImpl(conversation)); }
export function renameConversation(id: string, title: string) { return ordered(() => renameConversationImpl(id, title)); }
export function deleteConversation(id: string) { return ordered(() => deleteConversationImpl(id)); }
