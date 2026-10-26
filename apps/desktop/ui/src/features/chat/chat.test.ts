import { describe, expect, it } from "vitest";
import {
  MAX_MESSAGE_LENGTH,
  createAssistantMessage,
  createUserMessage,
  toChatTurns,
  validateDraft,
  type ChatMessage,
} from "./chat";

describe("validateDraft", () => {
  it("rejects an empty draft", () => {
    const result = validateDraft("   \n\t ");

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe("empty");
    }
  });

  it("trims surrounding whitespace and normalises CRLF", () => {
    expect(validateDraft("  fix the build\r\nplease  ")).toEqual({
      ok: true,
      text: "fix the build\nplease",
    });
  });

  it("accepts a message at the length limit", () => {
    expect(validateDraft("a".repeat(MAX_MESSAGE_LENGTH)).ok).toBe(true);
  });

  it("rejects a message over the length limit", () => {
    const result = validateDraft("a".repeat(MAX_MESSAGE_LENGTH + 1));

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe("too_long");
    }
  });
});

describe("message factories", () => {
  it("creates a completed user message", () => {
    const message = createUserMessage("hello", "u1", 1_000);

    expect(message).toEqual({
      id: "u1",
      role: "user",
      content: "hello",
      createdAt: 1_000,
      state: "complete",
    });
  });

  it("creates a streaming assistant message tied to a request", () => {
    const message = createAssistantMessage("a1", "req-1", 1_000);

    expect(message.role).toBe("assistant");
    expect(message.state).toBe("streaming");
    expect(message.requestId).toBe("req-1");
    expect(message.content).toBe("");
  });

  it("never contains canned model text", () => {
    const message = createAssistantMessage("a1", "req-1", 1_000);

    expect(message.content).toBe("");
  });
});

describe("toChatTurns", () => {
  const base: ChatMessage = {
    id: "1",
    role: "user",
    content: "hello",
    createdAt: 1,
    state: "complete",
  };

  it("maps visible messages to engine turns", () => {
    expect(
      toChatTurns([base, { ...base, id: "2", role: "assistant", content: "hi" }]),
    ).toEqual([
      { role: "user", content: "hello" },
      { role: "assistant", content: "hi" },
    ]);
  });

  it("keeps prepared image-to-3D reference tasks out of model turns", () => {
    const reference = {
      schemaVersion: 1 as const,
      referenceId: "0123456789abcdef0123456789abcdef",
      format: "image/png" as const,
      width: 512,
      height: 512,
      sizeBytes: 100,
      processingState: "validated" as const,
      validationErrors: [],
    };
    const localReferenceMessage: ChatMessage = {
      ...base,
      id: "reference-task",
      content: "Create a 3D asset from this image",
      localOnly: true,
      imageReference: reference,
      imageTo3dTask: {
        schemaVersion: 1,
        taskType: "image_to_3d",
        userRequest: "Create a 3D asset from this image",
        references: [reference],
        knownConstraints: [],
        missingInformation: [],
        analysisAvailable: false,
        nextStage: "awaiting_supported_vision_capability",
      },
    };
    expect(toChatTurns([base, localReferenceMessage])).toEqual([
      { role: "user", content: "hello" },
    ]);
  });

  it("drops empty, streaming-empty, and failed replies", () => {
    const turns = toChatTurns([
      base,
      { ...base, id: "2", role: "assistant", content: "", state: "streaming" },
      { ...base, id: "3", role: "assistant", content: "", state: "error" },
      { ...base, id: "4", role: "assistant", content: "kept", state: "stopped" },
    ]);

    expect(turns).toEqual([
      { role: "user", content: "hello" },
      { role: "assistant", content: "kept" },
    ]);
  });
});
