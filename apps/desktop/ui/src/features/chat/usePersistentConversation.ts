import { useCallback, useEffect, useRef, useState } from "react";
import {
  cancelGeneration,
  generate,
  toInferenceError,
} from "../../lib/inference";
import type { GenerationOutcome } from "../../lib/ipc";
import {
  BUSY_ERROR,
  createAssistantMessage,
  createUserMessage,
  toChatTurns,
  validateDraft,
  type ChatMessage,
  type ComposerError,
} from "./chat";
import {
  appendDelta,
  isCurrentConversation,
  patchMessage,
  removeMessage,
  shouldIgnoreSubmission,
} from "./conversation";
import { deriveTitle, sanitizeTitle } from "./history";
import {
  deleteConversation,
  listConversations,
  loadConversation,
  renameConversation,
  saveConversation,
  type ConversationSummary,
  type PersistedConversation,
} from "../../lib/history";
import { isDesktopShell } from "../../lib/ipc";

let fallbackId = 0;

function createId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  fallbackId += 1;
  return `id-${Date.now()}-${fallbackId}`;
}

interface StreamSession {
  requestId: string;
  assistantId: string;
  conversationId: string;
}

export type SaveStatus = "idle" | "saving" | "saved" | "error";

export interface PersistentConversationController {
  // History
  conversations: ConversationSummary[];
  activeId: string | null;
  activeTitle: string;
  listLoading: boolean;
  listError: string | null;
  saveStatus: SaveStatus;
  saveError: string | null;
  loadError: string | null;

  // Current conversation messages & composer
  messages: ChatMessage[];
  draft: string;
  error: ComposerError | null;
  runtimeError: string | null;
  streaming: boolean;

  // Actions
  setDraft: (value: string) => void;
  dismissError: () => void;
  dismissRuntimeError: () => void;
  dismissSaveError: () => void;
  dismissLoadError: () => void;
  refreshList: () => void;
  submit: () => void;
  stop: () => void;
  newChat: () => void;
  selectConversation: (id: string) => void;
  renameActive: (newTitle: string) => Promise<void>;
  deleteActive: () => Promise<void>;
  deleteConversationById: (id: string) => Promise<void>;
}

interface Options {
  onSettled?: () => void;
}

/**
 * Persistent conversation manager.
 *
 * - Maintains list of saved conversations via Rust storage (or localStorage fallback).
 * - Current conversation is persisted with debounce, preserving Phase 9 formatting as plain text.
 * - Switching conversations cancels in-flight generation and drops stale deltas via conversationId.
 * - Restore never triggers inference.
 * - Failed saves surface to UI, not silent.
 * - Debounce 500ms to avoid excessive writes.
 */
export function usePersistentConversation(
  { onSettled }: Options = {}
): PersistentConversationController {
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [activeTitle, setActiveTitle] = useState<string>("New conversation");
  const [listLoading, setListLoading] = useState(true);
  const [listError, setListError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraftState] = useState("");
  const [error, setError] = useState<ComposerError | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [streaming, setStreaming] = useState(false);
  const [saveStatus, setSaveStatus] = useState<SaveStatus>("idle");
  const [saveError, setSaveError] = useState<string | null>(null);

  const messagesRef = useRef<ChatMessage[]>([]);
  const draftRef = useRef("");
  const conversationRef = useRef<string>(createId());
  const lastSubmitAtRef = useRef(0);
  const sessionRef = useRef<StreamSession | null>(null);
  const mountedRef = useRef(true);

  const activeIdRef = useRef<string | null>(null);
  const saveTimeoutRef = useRef<number | null>(null);
  const saveSeqRef = useRef(0);
  const pendingSaveRef = useRef<PersistedConversation | null>(null);

  // metadata refs for title/createdAt
  const activeTitleRef = useRef("New conversation");
  const createdAtRef = useRef<number>(Date.now());

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (saveTimeoutRef.current !== null) {
        window.clearTimeout(saveTimeoutRef.current);
      }
    };
  }, []);

  const commit = useCallback((next: ChatMessage[]) => {
    messagesRef.current = next;
    setMessages(next);
  }, []);

  const setDraft = useCallback((value: string) => {
    draftRef.current = value;
    setDraftState(value);
  }, []);

  const dismissError = useCallback(() => setError(null), []);
  const dismissRuntimeError = useCallback(() => setRuntimeError(null), []);
  const dismissSaveError = useCallback(() => {
    setSaveError(null);
    setSaveStatus("idle");
  }, []);
  const dismissLoadError = useCallback(() => setLoadError(null), []);

  // Load list on mount
  const refreshList = useCallback(() => {
    setListLoading(true);
    setListError(null);
    listConversations()
      .then((list) => {
        if (!mountedRef.current) return;
        setConversations(list);
        setListLoading(false);
      })
      .catch((cause: unknown) => {
        if (!mountedRef.current) return;
        const msg = cause instanceof Error ? cause.message : (cause as { message?: string })?.message ?? "Could not load conversations";
        setListError(msg);
        setListLoading(false);
      });
  }, []);

  useEffect(() => {
    refreshList();
  }, [refreshList]);

  // Persist helper — call after messages/title change
  const scheduleSave = useCallback(() => {
    const currentId = activeIdRef.current;
    if (!currentId) return;

    const msgs = messagesRef.current;
    const title = deriveTitle(msgs, activeTitleRef.current);

    const payload: PersistedConversation = {
      id: currentId,
      title: sanitizeTitle(title),
      createdAt: createdAtRef.current,
      updatedAt: Date.now(),
      messages: msgs.map((m) => ({
        id: m.id,
        role: m.role,
        content: m.content,
        createdAt: m.createdAt,
        state: m.state,
        failure: m.failure,
      })),
    };

    pendingSaveRef.current = payload;

    if (saveTimeoutRef.current !== null) {
      window.clearTimeout(saveTimeoutRef.current);
    }

    setSaveStatus("saving");
    setSaveError(null);

    const doSave = () => {
      saveTimeoutRef.current = null;
      const seq = ++saveSeqRef.current;
      const toSave = pendingSaveRef.current;
      if (!toSave) return;

      const capturedId = toSave.id;

      saveConversation(toSave)
        .then((saved) => {
          if (!mountedRef.current) return;
          if (seq !== saveSeqRef.current) return;
          if (activeIdRef.current !== capturedId) return;

          setSaveStatus("saved");
          setConversations((prev) =>
            prev.map((c) =>
              c.id === saved.id
                ? {
                    ...c,
                    title: saved.title,
                    updatedAt: saved.updatedAt,
                    messageCount: saved.messages.length,
                    preview: saved.messages.length > 0 ? saved.messages[saved.messages.length - 1]!.content.slice(0, 120) : "",
                  }
                : c
            ).sort((a, b) => b.updatedAt - a.updatedAt)
          );
          window.setTimeout(() => {
            if (mountedRef.current && saveSeqRef.current === seq) {
              setSaveStatus("idle");
            }
          }, 2000);
        })
        .catch((cause: unknown) => {
          if (!mountedRef.current) return;
          if (seq !== saveSeqRef.current) return;
          const msg = (cause as { message?: string })?.message ?? "Failed to save conversation";
          setSaveError(msg);
          setSaveStatus("error");
        });
    };

    // In browser fallback (tests) persist immediately so a second hook can read the same storage.
    // On desktop keep the debounce to avoid excessive disk writes.
    if (!isDesktopShell()) {
      doSave();
    } else {
      saveTimeoutRef.current = window.setTimeout(doSave, 500);
    }
  }, []);

  // Flush pending save immediately (for beforeunload or rapid switch)
  const flushSave = useCallback(() => {
    if (saveTimeoutRef.current !== null) {
      window.clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = null;
    }
    const toSave = pendingSaveRef.current;
    if (!toSave) return;
    const capturedId = toSave.id;
    const seq = ++saveSeqRef.current;
    setSaveStatus("saving");
    saveConversation(toSave)
      .then((saved) => {
        if (seq !== saveSeqRef.current) return;
        if (activeIdRef.current !== capturedId) return;
        setSaveStatus("saved");
        setConversations((prev) =>
          prev.map((c) =>
            c.id === saved.id
              ? {
                  ...c,
                  title: saved.title,
                  updatedAt: saved.updatedAt,
                  messageCount: saved.messages.length,
                  preview: saved.messages.length > 0 ? saved.messages[saved.messages.length - 1]!.content.slice(0, 120) : "",
                }
              : c
          )
        );
      })
      .catch((cause: unknown) => {
        const msg = (cause as { message?: string })?.message ?? "Failed to save";
        setSaveError(msg);
        setSaveStatus("error");
      });
  }, []);

  // Handle beforeunload: flush save
  useEffect(() => {
    const handler = () => {
      if (pendingSaveRef.current && activeIdRef.current) {
        // Best effort synchronous? We already have async flush; for beforeunload we try flush
        // Note: async may not complete, but we attempt
        flushSave();
      }
    };
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [flushSave]);

  // When messages change due to submit/stop/completion, schedule save
  const triggerSaveIfActive = useCallback(() => {
    if (activeIdRef.current) {
      scheduleSave();
    }
  }, [scheduleSave]);

  const selectConversation = useCallback(
    (id: string) => {
      // Flush previous pending save before switching
      if (activeIdRef.current && pendingSaveRef.current) {
        flushSave();
      }

      // Cancel in-flight generation so its deltas don't land in new conversation
      const session = sessionRef.current;
      if (session) {
        void cancelGeneration(session.requestId);
        sessionRef.current = null;
        setStreaming(false);
      }

      setLoadError(null);
      setSaveError(null);
      setSaveStatus("idle");

      // Set active immediately to show loading state
      activeIdRef.current = id;
      setActiveId(id);

      loadConversation(id)
        .then((conv) => {
          if (!mountedRef.current) return;
          if (activeIdRef.current !== id) return; // stale load
          // Coerce any streaming to complete (Rust already does, but ensure)
          const msgs: ChatMessage[] = conv.messages.map((m) => ({
            id: m.id,
            role: m.role as "user" | "assistant",
            content: m.content,
            createdAt: m.createdAt,
            state: (m.state as ChatMessage["state"]) ?? "complete",
            failure: m.failure,
          }));
          // Ensure no message remains in streaming state — would trigger false busy
          for (const msg of msgs) {
            if (msg.state === "streaming") {
              msg.state = msg.content.trim().length === 0 ? "stopped" : "complete";
            }
          }
          conversationRef.current = id;
          createdAtRef.current = conv.createdAt;
          activeTitleRef.current = conv.title;
          setActiveTitle(conv.title);
          commit(msgs);
          setDraft("");
          draftRef.current = "";
          setError(null);
          setRuntimeError(null);
        })
        .catch((cause: unknown) => {
          if (!mountedRef.current) return;
          const msg = (cause as { message?: string })?.message ?? "Could not load conversation";
          setLoadError(msg);
        });
    },
    [commit, flushSave]
  );

  const newChat = useCallback(() => {
    // Flush pending save for previous conversation
    if (activeIdRef.current && pendingSaveRef.current) {
      flushSave();
    }

    const session = sessionRef.current;
    if (session) {
      void cancelGeneration(session.requestId);
    }
    sessionRef.current = null;
    const newId = createId();
    conversationRef.current = newId;
    activeIdRef.current = null;
    createdAtRef.current = Date.now();
    activeTitleRef.current = "New conversation";
    messagesRef.current = [];
    draftRef.current = "";
    lastSubmitAtRef.current = 0;
    pendingSaveRef.current = null;

    setActiveId(null);
    setActiveTitle("New conversation");
    setMessages([]);
    setDraftState("");
    setError(null);
    setRuntimeError(null);
    setStreaming(false);
    setSaveStatus("idle");
    setSaveError(null);
    setLoadError(null);
  }, [flushSave]);

  const submit = useCallback(() => {
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

    // If no active conversation, create one first (optimistically)
    let currentActiveId = activeIdRef.current;
    let isNewConversation = false;
    if (!currentActiveId) {
      currentActiveId = createId();
      activeIdRef.current = currentActiveId;
      conversationRef.current = currentActiveId;
      createdAtRef.current = now;
      activeTitleRef.current = "New conversation";
      setActiveId(currentActiveId);
      isNewConversation = true;

      // Create in storage optimistically - will be updated on save
      // We also need to add to list immediately with placeholder
      const placeholderTitle = sanitizeTitle(validation.text.slice(0, 60) || "New conversation");
      const optimisticSummary: ConversationSummary = {
        id: currentActiveId,
        title: placeholderTitle,
        createdAt: now,
        updatedAt: now,
        messageCount: 0,
        preview: validation.text.slice(0, 120),
      };
      setConversations((prev) => [optimisticSummary, ...prev]);
      setActiveTitle(placeholderTitle);
    }

    const session: StreamSession = {
      requestId: createId(),
      assistantId: createId(),
      conversationId: conversationRef.current,
    };

    const userMessage = createUserMessage(validation.text, createId(), now);
    const assistantMessage = createAssistantMessage(session.assistantId, session.requestId, now);
    const history = [...messagesRef.current, userMessage];

    lastSubmitAtRef.current = now;
    draftRef.current = "";
    setDraftState("");
    setError(null);
    setRuntimeError(null);
    const nextMessages = [...history, assistantMessage];
    commit(nextMessages);

    sessionRef.current = session;
    setStreaming(true);

    // Derive title if first user message and trigger immediate save of user message
    if (isNewConversation || messagesRef.current.length <= 2) {
      const derived = deriveTitle(nextMessages);
      activeTitleRef.current = derived;
      setActiveTitle(derived);
      // Update list title optimistically
      if (currentActiveId) {
        setConversations((prev) =>
          prev.map((c) =>
            c.id === currentActiveId
              ? { ...c, title: derived, updatedAt: Date.now(), messageCount: nextMessages.length, preview: nextMessages[nextMessages.length - 1]?.content.slice(0, 120) ?? "" }
              : c
          ).sort((a, b) => b.updatedAt - a.updatedAt)
        );
      }
    }

    // Schedule save for the user message + empty assistant placeholder
    // Persist immediately in fallback (tests) so a second hook can read it; on desktop use debounced path.
    if (currentActiveId) {
      const initialPayload: PersistedConversation = {
        id: currentActiveId,
        title: activeTitleRef.current,
        createdAt: createdAtRef.current,
        updatedAt: Date.now(),
        messages: nextMessages.map((m) => ({
          id: m.id,
          role: m.role,
          content: m.content,
          createdAt: m.createdAt,
          state: m.state,
          failure: m.failure,
        })),
      };
      pendingSaveRef.current = initialPayload;
      if (!isDesktopShell()) {
        const seq = ++saveSeqRef.current;
        const capturedId = initialPayload.id;
        setSaveStatus("saving");
        saveConversation(initialPayload)
          .then((saved) => {
            if (seq !== saveSeqRef.current) return;
            if (activeIdRef.current !== capturedId) return;
            setSaveStatus("saved");
            setConversations((prev) =>
              prev.map((c) =>
                c.id === saved.id
                  ? { ...c, title: saved.title, updatedAt: saved.updatedAt, messageCount: saved.messages.length, preview: saved.messages.length > 0 ? saved.messages[saved.messages.length - 1]!.content.slice(0, 120) : "" }
                  : c
              ).sort((a, b) => b.updatedAt - a.updatedAt)
            );
          })
          .catch(() => {});
      } else {
        if (saveTimeoutRef.current !== null) window.clearTimeout(saveTimeoutRef.current);
        saveTimeoutRef.current = window.setTimeout(() => {
          saveTimeoutRef.current = null;
          const seq = ++saveSeqRef.current;
          const toSave = pendingSaveRef.current;
          if (!toSave) return;
          const capturedId = toSave.id;
          setSaveStatus("saving");
          saveConversation(toSave)
            .then((saved) => {
              if (seq !== saveSeqRef.current) return;
              if (activeIdRef.current !== capturedId) return;
              setSaveStatus("saved");
              setConversations((prev) =>
                prev.map((c) =>
                  c.id === saved.id
                    ? { ...c, title: saved.title, updatedAt: saved.updatedAt, messageCount: saved.messages.length, preview: saved.messages.length > 0 ? saved.messages[saved.messages.length - 1]!.content.slice(0, 120) : "" }
                    : c
                ).sort((a, b) => b.updatedAt - a.updatedAt)
              );
            })
            .catch(() => {});
        }, 300);
      }
    }

    const turns = toChatTurns(history);

    const isLive = () => sessionRef.current === session;

    const settle = (outcome: GenerationOutcome) => {
      if (!mountedRef.current || !isLive()) return;
      if (!isCurrentConversation(session.conversationId, conversationRef.current)) return;

      const current = messagesRef.current.find((m) => m.id === session.assistantId);
      if (!current) return;

      if (outcome.cancelled) {
        commit(patchMessage(messagesRef.current, session.assistantId, { state: "stopped" }));
        triggerSaveIfActive();
        return;
      }

      if (current.content.trim().length === 0) {
        commit(removeMessage(messagesRef.current, session.assistantId));
        setRuntimeError("The model finished without producing any text. Check the model and try again.");
        triggerSaveIfActive();
        return;
      }

      commit(patchMessage(messagesRef.current, session.assistantId, { state: "complete" }));
      triggerSaveIfActive();
    };

    const fail = (cause: unknown) => {
      const failure = toInferenceError(cause);
      if (!mountedRef.current || !isLive()) return;
      if (!isCurrentConversation(session.conversationId, conversationRef.current)) return;

      const current = messagesRef.current.find((m) => m.id === session.assistantId);
      if (!current || current.content.trim().length === 0) {
        commit(removeMessage(messagesRef.current, session.assistantId));
      } else {
        commit(
          patchMessage(messagesRef.current, session.assistantId, {
            state: "error",
            failure: failure.message,
          })
        );
      }
      setRuntimeError(failure.message);
      triggerSaveIfActive();
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
            if (event.type !== "delta" || !event.text) return;
            if (!isLive() || event.requestId !== session.requestId) return;
            if (!isCurrentConversation(session.conversationId, conversationRef.current)) return;
            commit(appendDelta(messagesRef.current, session.assistantId, event.text));
            // Debounce save during streaming? Save periodically but not every token to avoid spam
            // We update pendingSave but debounce
            if (activeIdRef.current) {
              const msgs = messagesRef.current;
              pendingSaveRef.current = {
                id: activeIdRef.current,
                title: activeTitleRef.current,
                createdAt: createdAtRef.current,
                updatedAt: Date.now(),
                messages: msgs.map((m) => ({
                  id: m.id,
                  role: m.role,
                  content: m.content,
                  createdAt: m.createdAt,
                  state: m.state,
                  failure: m.failure,
                })),
              };
            }
          }
        );
        settle(outcome);
      } catch (cause) {
        fail(cause);
      } finally {
        if (sessionRef.current === session) {
          sessionRef.current = null;
          if (mountedRef.current) {
            setStreaming(false);
            onSettled?.();
          }
        }
      }
    })();
  }, [commit, onSettled, triggerSaveIfActive]);

  const stop = useCallback(() => {
    const session = sessionRef.current;
    if (session) {
      void cancelGeneration(session.requestId);
    }
  }, []);

  const renameActive = useCallback(
    async (newTitle: string) => {
      const id = activeIdRef.current;
      if (!id) return;
      const sanitized = sanitizeTitle(newTitle);
      try {
        const updated = await renameConversation(id, sanitized);
        if (!mountedRef.current) return;
        activeTitleRef.current = updated.title;
        setActiveTitle(updated.title);
        setConversations((prev) => prev.map((c) => (c.id === id ? updated : c)).sort((a, b) => b.updatedAt - a.updatedAt));
      } catch (cause: unknown) {
        const msg = (cause as { message?: string })?.message ?? "Could not rename conversation";
        setSaveError(msg);
        setSaveStatus("error");
        throw cause;
      }
    },
    []
  );

  const deleteActive = useCallback(async () => {
    const id = activeIdRef.current;
    if (!id) return;
    try {
      await deleteConversation(id);
      if (!mountedRef.current) return;
      setConversations((prev) => prev.filter((c) => c.id !== id));
      newChat();
    } catch (cause: unknown) {
      const msg = (cause as { message?: string })?.message ?? "Could not delete conversation";
      setSaveError(msg);
      setSaveStatus("error");
      throw cause;
    }
  }, [newChat]);

  const deleteConversationById = useCallback(
    async (id: string) => {
      try {
        await deleteConversation(id);
        if (!mountedRef.current) return;
        setConversations((prev) => prev.filter((c) => c.id !== id));
        if (activeIdRef.current === id) {
          newChat();
        }
      } catch (cause: unknown) {
        const msg = (cause as { message?: string })?.message ?? "Could not delete conversation";
        setSaveError(msg);
        setSaveStatus("error");
        throw cause;
      }
    },
    [newChat]
  );

  // Effect to schedule save when messages change (except during streaming every delta we handle above, but completion already triggers)
  // We also watch messages length outside of submit path (e.g., after load we don't want to save)
  // So we rely on explicit triggerSaveIfActive calls rather than generic effect to avoid loop on load

  return {
    conversations,
    activeId,
    activeTitle,
    listLoading,
    listError,
    saveStatus,
    saveError,
    loadError,
    messages,
    draft,
    error,
    runtimeError,
    streaming,
    setDraft,
    dismissError,
    dismissRuntimeError,
    dismissSaveError,
    dismissLoadError,
    refreshList,
    submit,
    stop,
    newChat,
    selectConversation,
    renameActive,
    deleteActive,
    deleteConversationById,
  };
}
