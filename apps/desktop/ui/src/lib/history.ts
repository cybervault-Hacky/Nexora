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

function fallbackLoad(): FallbackFile {
  try {
    if (typeof localStorage === "undefined") return { version: 1, conversations: [] };
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { version: 1, conversations: [] };
    const parsed = JSON.parse(raw) as FallbackFile;
    if (!parsed || typeof parsed.version !== "number" || !Array.isArray(parsed.conversations)) {
      return { version: 1, conversations: [] };
    }
    return parsed;
  } catch {
    return { version: 1, conversations: [] };
  }
}

function fallbackSave(file: FallbackFile): void {
  try {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(STORAGE_KEY, JSON.stringify(file));
  } catch {
    // ignore quota errors in fallback
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

export async function listConversations(): Promise<ConversationSummary[]> {
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
  try {
    return await call<ConversationSummary[]>("list_conversations");
  } catch {
    // In tests isDesktopShell may be mocked true but invoke not available — fall back
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
}

export async function loadConversation(id: string): Promise<PersistedConversation> {
  if (!isDesktopShell()) {
    const file = fallbackLoad();
    const conv = file.conversations.find((c) => c.id === id);
    if (!conv) throw toHistoryError({ code: "not_found", message: `Conversation ${id} not found` });
    const messages = conv.messages.map((m) => {
      if (m.state === "streaming") {
        return { ...m, state: m.content.trim().length === 0 ? "stopped" : "complete" };
      }
      return m;
    });
    return { ...conv, messages };
  }
  try {
    return await call<PersistedConversation>("load_conversation", { id });
  } catch {
    const file = fallbackLoad();
    const conv = file.conversations.find((c) => c.id === id);
    if (!conv) throw toHistoryError({ code: "not_found", message: `Conversation ${id} not found` });
    const messages = conv.messages.map((m) => {
      if (m.state === "streaming") {
        return { ...m, state: m.content.trim().length === 0 ? "stopped" : "complete" };
      }
      return m;
    });
    return { ...conv, messages };
  }
}

export async function createConversation(title?: string): Promise<PersistedConversation> {
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
  try {
    return await call<PersistedConversation>("create_conversation", { title: title ?? null });
  } catch {
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
}

export async function createConversationWithId(id: string, title?: string): Promise<PersistedConversation> {
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
  try {
    return await call<PersistedConversation>("create_conversation", { id, title: title ?? null });
  } catch {
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
}

export async function saveConversation(conversation: PersistedConversation): Promise<PersistedConversation> {
  // Normalize streaming state before save, mirroring Rust logic
  const normalized: PersistedConversation = {
    ...conversation,
    messages: conversation.messages.map((m) => {
      if (m.state === "streaming") {
        return { ...m, state: m.content.trim().length === 0 ? "stopped" : "complete" };
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
  try {
    return await call<PersistedConversation>("save_conversation", { conversation: normalized });
  } catch {
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
}

export async function renameConversation(id: string, title: string): Promise<ConversationSummary> {
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
  try {
    return await call<ConversationSummary>("rename_conversation", { id, title });
  } catch {
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
}

export async function deleteConversation(id: string): Promise<boolean> {
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
  try {
    return await call<boolean>("delete_conversation", { id });
  } catch {
    const file = fallbackLoad();
    const before = file.conversations.length;
    file.conversations = file.conversations.filter((c) => c.id !== id);
    if (file.conversations.length === before) {
      throw toHistoryError({ code: "not_found", message: `Conversation ${id} not found` });
    }
    fallbackSave(file);
    return true;
  }
}

// Utility for tests: clear fallback storage
export function clearFallbackStorage(): void {
  try {
    if (typeof localStorage !== "undefined") localStorage.removeItem(STORAGE_KEY);
  } catch {
    // ignore
  }
}
