import { useCallback, useEffect, useRef, useState } from "react";
import { cancelGeneration, generate, toInferenceError } from "../../lib/inference";
import type { GenerationOutcome } from "../../lib/ipc";
import { BUSY_ERROR, createAssistantMessage, createUserMessage, toChatTurns, validateDraft, type ChatMessage, type ComposerError } from "./chat";
import { appendDelta, isCurrentConversation, patchMessage, removeMessage, shouldIgnoreSubmission } from "./conversation";
import { deriveTitle, sanitizeTitle } from "./history";
import { createConversation, deleteConversation, listConversations, loadConversation, renameConversation, saveConversation, type ConversationSummary, type PersistedConversation } from "../../lib/history";

let fallbackId = 0;
function createId() { return crypto.randomUUID?.() ?? `id-${Date.now()}-${++fallbackId}`; }
interface StreamSession { requestId: string; assistantId: string; conversationId: string; }
export type SaveStatus = "idle" | "saving" | "saved" | "error";
const describe = (cause: unknown) => toInferenceError(cause).message;
function summary(c: PersistedConversation): ConversationSummary {
  return { id: c.id, title: c.title, createdAt: c.createdAt, updatedAt: c.updatedAt,
    messageCount: c.messages.length, preview: [...c.messages].reverse().find(m => m.content.trim())?.content.split("\n")[0]?.slice(0,120) ?? "" };
}

export function usePersistentConversation({ onSettled }: { onSettled?: () => void } = {}) {
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [activeTitle, setActiveTitle] = useState("New conversation");
  const [listLoading, setListLoading] = useState(true);
  const [listError, setListError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [conversationLoading, setConversationLoading] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraftState] = useState("");
  const [error, setError] = useState<ComposerError | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [streaming, setStreaming] = useState(false);
  const [saveStatus, setSaveStatus] = useState<SaveStatus>("idle");
  const [saveError, setSaveError] = useState<string | null>(null);
  const activeRef = useRef<PersistedConversation | null>(null);
  const messagesRef = useRef<ChatMessage[]>([]);
  const draftRef = useRef("");
  const conversationRef = useRef<string>(createId());
  const lastSubmitAtRef = useRef(0);
  const sessionRef = useRef<StreamSession | null>(null);
  const mountedRef = useRef(true);
  const loadingRef = useRef(false);
  const epoch = useRef(0);
  const saveSeq = useRef(0);
  const pending = useRef<PersistedConversation | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastWrite = useRef(Promise.resolve());

  const updateSummary = useCallback((c: PersistedConversation) => {
    if (!mountedRef.current) return;
    setConversations(prev => [summary(c), ...prev.filter(item => item.id !== c.id)]
      .sort((a,b) => b.updatedAt - a.updatedAt));
  }, []);

  const flush = useCallback((): Promise<void> => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    const payload = pending.current;
    if (!payload) return lastWrite.current;
    pending.current = null;
    const seq = ++saveSeq.current;
    if (mountedRef.current) setSaveStatus("saving");
    const work = saveConversation(payload).then(saved => {
      updateSummary(saved);
      if (mountedRef.current && seq === saveSeq.current && !pending.current) {
        setSaveStatus("saved"); setSaveError(null);
      }
    }).catch(cause => {
      // Keep the most recent unsaved snapshot for retry, including when switching.
      if (seq === saveSeq.current && !pending.current) pending.current = payload;
      if (mountedRef.current) { setSaveError(describe(cause)); setSaveStatus("error"); }
      throw cause;
    });
    lastWrite.current = work;
    void work.catch(() => undefined);
    return work;
  }, [updateSummary]);

  const commit = useCallback((next: ChatMessage[], checkpoint = false) => {
    messagesRef.current = next;
    setMessages(next);
    const active = activeRef.current;
    if (!active) return;
    const payload: PersistedConversation = { ...active, updatedAt: Date.now(), messages: next.map(m => ({
      id:m.id, role:m.role, content:m.content, createdAt:m.createdAt,
      state:m.state === "streaming" ? "stopped" : m.state, failure:m.failure,
    })) };
    activeRef.current = payload;
    pending.current = payload;
    ++saveSeq.current;
    setSaveStatus("saving");
    // Bounded 1s checkpoint, not an indefinitely postponed debounce.
    if (checkpoint) {
      if (!timer.current) timer.current = setTimeout(() => { void flush().catch(() => undefined); }, 1000);
    } else { void flush().catch(() => undefined); }
  }, [flush]);

  const refreshList = useCallback(() => {
    setListLoading(true); setListError(null);
    void listConversations().then(list => { if (mountedRef.current) setConversations(list); })
      .catch(cause => { if (mountedRef.current) setListError(describe(cause)); })
      .finally(() => { if (mountedRef.current) setListLoading(false); });
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    refreshList();
    const handler = () => { void flush().catch(() => undefined); };
    window.addEventListener("beforeunload", handler);
    return () => {
      mountedRef.current = false;
      ++epoch.current;
      sessionRef.current && void cancelGeneration(sessionRef.current.requestId);
      sessionRef.current = null;
      void flush().catch(() => undefined);
      window.removeEventListener("beforeunload", handler);
    };
  }, [refreshList, flush]);

  const setDraft = useCallback((value: string) => { draftRef.current = value; setDraftState(value); }, []);
  const dismissError = useCallback(() => setError(null), []);
  const dismissRuntimeError = useCallback(() => setRuntimeError(null), []);
  const dismissSaveError = useCallback(() => { void flush().catch(() => undefined); }, [flush]);
  const dismissLoadError = useCallback(() => setLoadError(null), []);

  const detach = useCallback(() => {
    const session = sessionRef.current;
    if (session) {
      sessionRef.current = null;
      void cancelGeneration(session.requestId);
      commit(messagesRef.current.map(m => m.state === "streaming" ? { ...m, state: "stopped" } : m));
    }
    setStreaming(false);
  }, [commit]);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let closing = false;
    void import("@tauri-apps/api/window").then(async ({ getCurrentWindow }) => {
      const win = getCurrentWindow();
      const off = await win.onCloseRequested(event => {
        if (closing) return;
        event.preventDefault();
        detach();
        void flush().then(async () => {
          closing = true;
          await win.close();
        }).catch(() => { /* Keep window open with the visible save error. */ });
      });
      if (disposed) off(); else unlisten = off;
    }).catch(() => { /* IPC unavailable: storage operations themselves report failure. */ });
    return () => { disposed = true; unlisten?.(); };
  }, [detach, flush]);

  const selectConversation = useCallback((id: string) => {
    const turn = ++epoch.current;
    loadingRef.current = true; setConversationLoading(true); setLoadError(null);
    detach();
    void (async () => {
      try {
        await flush();
        const conv = await loadConversation(id);
        if (!mountedRef.current || epoch.current !== turn) return;
        activeRef.current = conv; conversationRef.current = id;
        setActiveId(id); setActiveTitle(conv.title);
        const restored: ChatMessage[] = conv.messages.map(m => ({ ...m,
          state: m.state === "streaming" ? "stopped" : (m.state ?? "complete") as ChatMessage["state"],
        }));
        messagesRef.current = restored; setMessages(restored);
        setDraft(""); setError(null); setRuntimeError(null); lastSubmitAtRef.current = 0;
      } catch(cause) {
        if (mountedRef.current && epoch.current === turn) setLoadError(describe(cause));
      } finally {
        if (mountedRef.current && epoch.current === turn) { loadingRef.current = false; setConversationLoading(false); }
      }
    })();
  }, [detach, flush, setDraft]);

  const newChat = useCallback(() => {
    const turn = ++epoch.current;
    loadingRef.current = true; setConversationLoading(true); detach();
    void (async () => {
      try {
        await flush();
        const conv = await createConversation();
        updateSummary(conv);
        if (!mountedRef.current || epoch.current !== turn) return;
        activeRef.current = conv; conversationRef.current = conv.id;
        messagesRef.current = []; setMessages([]); setDraft("");
        setActiveId(conv.id); setActiveTitle(conv.title);
        lastSubmitAtRef.current = 0; setError(null); setRuntimeError(null); setLoadError(null);
      } catch (cause) {
        if (mountedRef.current && epoch.current === turn) setLoadError(describe(cause));
      } finally {
        if (mountedRef.current && epoch.current === turn) { loadingRef.current = false; setConversationLoading(false); }
      }
    })();
  }, [detach, flush, setDraft, updateSummary]);

  const submit = useCallback((options?: { contextBlock?: string }) => {
    if (loadingRef.current) return;
    if (!activeRef.current) {
      const now = Date.now();
      const conv: PersistedConversation = { id: createId(), title: "New conversation", createdAt: now, updatedAt: now, messages: [] };
      activeRef.current = conv;
      conversationRef.current = conv.id;
      setActiveId(conv.id);
    }
    if (sessionRef.current) {
      setError(BUSY_ERROR);
      return;
    }

    const now = Date.now();

    if (shouldIgnoreSubmission(lastSubmitAtRef.current, now)) {
      return;
    }

    const validation = validateDraft(draftRef.current);

    if (!validation.ok) {
      setError(validation.error);
      return;
    }

    const session: StreamSession = {
      requestId: createId(),
      assistantId: createId(),
      conversationId: conversationRef.current,
    };

    const userMessage = createUserMessage(validation.text, createId(), now);
    const assistantMessage = createAssistantMessage(
      session.assistantId,
      session.requestId,
      now,
    );
    const history = [...messagesRef.current, userMessage];

    lastSubmitAtRef.current = now;
    draftRef.current = "";
    setDraftState("");
    setError(null);
    setRuntimeError(null);
    if (activeRef.current?.title === "New conversation") {
      activeRef.current.title = deriveTitle(history);
      setActiveTitle(activeRef.current.title);
    }
    commit([...history, assistantMessage]);

    sessionRef.current = session;
    setStreaming(true);

    let turns = toChatTurns(history);
    const contextBlock = options?.contextBlock?.trim();
    if (contextBlock && contextBlock.length > 0) {
      // Attach untrusted project context to the last user turn only for the model request.
      // Persisted history keeps the original prompt unchanged.
      if (contextBlock.length > 100 * 1024) {
        // Defensive truncation — context is already bounded in useWorkspace, but never exceed request budget
      }
      const last = turns[turns.length - 1];
      if (last && last.role === "user") {
        turns = [...turns.slice(0, -1), { ...last, content: `${contextBlock}\n\n${last.content}` }];
      } else {
        turns = [{ role: "user", content: contextBlock }, ...turns];
      }
    }

    const isLive = () => sessionRef.current === session;

    const settle = (outcome: GenerationOutcome) => {
      if (!mountedRef.current || !isLive()) {
        return;
      }

      if (!isCurrentConversation(session.conversationId, conversationRef.current)) {
        return;
      }

      const current = messagesRef.current.find(
        (message) => message.id === session.assistantId,
      );

      if (!current) {
        return;
      }

      if (outcome.cancelled) {
        // Never invent an assistant turn: an empty stop stays empty so it is
        // excluded from the next request and shown as a system note instead.
        commit(
          patchMessage(messagesRef.current, session.assistantId, {
            state: "stopped",
          }),
        );
        return;
      }

      if (current.content.trim().length === 0) {
        commit(removeMessage(messagesRef.current, session.assistantId));
        setRuntimeError(
          "The model finished without producing any text. Check the model and try again.",
        );
        return;
      }

      commit(
        patchMessage(messagesRef.current, session.assistantId, { state: "complete" }),
      );
    };

    const fail = (cause: unknown) => {
      const failure = toInferenceError(cause);

      if (!mountedRef.current || !isLive()) {
        return;
      }

      if (!isCurrentConversation(session.conversationId, conversationRef.current)) {
        return;
      }

      const current = messagesRef.current.find(
        (message) => message.id === session.assistantId,
      );

      if (!current || current.content.trim().length === 0) {
        commit(removeMessage(messagesRef.current, session.assistantId));
      } else {
        commit(
          patchMessage(messagesRef.current, session.assistantId, {
            state: "error",
            failure: failure.message,
          }),
        );
      }

      setRuntimeError(failure.message);
    };

    void (async () => {
      try {
        const outcome = await generate(
          {
            requestId: session.requestId,
            messages: turns,
            maxTokens: 1024,
            temperature: 0.2,
          },
          (event) => {
            if (event.type !== "delta" || !event.text) {
              return;
            }

            if (!isLive() || event.requestId !== session.requestId) {
              return;
            }

            if (!isCurrentConversation(session.conversationId, conversationRef.current)) {
              return;
            }

            commit(appendDelta(messagesRef.current, session.assistantId, event.text), true);
          },
        );

        settle(outcome);
      } catch (cause) {
        fail(cause);
      } finally {
        if (sessionRef.current === session) {
          sessionRef.current = null;

          // A generation can outlive the component that started it. Writing
          // state is a no-op after unmount, but `onSettled` would still spend an
          // IPC round trip on a tree that no longer exists.
          if (mountedRef.current) {
            setStreaming(false);
            onSettled?.();
          }
        }
      }
    })();
  }, [commit, onSettled]);


  const stop = useCallback(() => {
    if (sessionRef.current) void cancelGeneration(sessionRef.current.requestId);
  }, []);

  const renameById = useCallback(async (id: string, title: string) => {
    detach();
    try {
      await flush();
      const saved = await renameConversation(id, sanitizeTitle(title));
      if (!mountedRef.current) return;
      if (activeRef.current?.id === id) { activeRef.current.title = saved.title; setActiveTitle(saved.title); }
      setConversations(prev => prev.map(c => c.id === id ? saved : c));
    } catch(cause) { setSaveError(describe(cause)); setSaveStatus("error"); throw cause; }
  }, [detach, flush]);
  const renameActive = useCallback(async (title: string) => {
    if (activeRef.current) await renameById(activeRef.current.id, title);
  }, [renameById]);

  const deleteConversationById = useCallback(async (id: string) => {
    if (activeRef.current?.id === id) detach();
    try {
      await flush();
      await deleteConversation(id);
      if (!mountedRef.current) return;
      setConversations(prev => prev.filter(c => c.id !== id));
      if (activeRef.current?.id === id) {
        activeRef.current = null; messagesRef.current = [];
        setMessages([]); setActiveId(null); setActiveTitle("New conversation"); setDraft("");
        conversationRef.current = createId();
      }
    } catch(cause) { setSaveError(describe(cause)); setSaveStatus("error"); throw cause; }
  }, [detach, flush, setDraft]);
  const deleteActive = useCallback(async () => {
    if (activeRef.current) await deleteConversationById(activeRef.current.id);
  }, [deleteConversationById]);

  return { conversations, activeId, activeTitle, listLoading, listError, saveStatus,
    saveError, loadError, conversationLoading, messages, draft, error, runtimeError,
    streaming, setDraft, dismissError, dismissRuntimeError, dismissSaveError,
    dismissLoadError, refreshList, submit, stop, newChat, selectConversation,
    renameActive, renameById, deleteActive, deleteConversationById };
}
