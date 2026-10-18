import { describe, expect, it } from "vitest";
import { parseAndValidatePatchResponse, validateInstruction } from "./patchContract";

const original = "let a = 1;\n";

describe("patchContract — valid", () => {
  it("accepts valid single-file proposal", () => {
    const json = JSON.stringify({ version: 1, targetPath: "src/app.ts", proposedContent: "let a = 2;\n", explanation: "fix" });
    const res = parseAndValidatePatchResponse(json, "src/app.ts", original);
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.contract.targetPath).toBe("src/app.ts");
      expect(res.contract.proposedContent).toBe("let a = 2;\n");
      expect(res.contract.explanation).toBe("fix");
    }
  });

  it("accepts without explanation", () => {
    const json = JSON.stringify({ version: 1, targetPath: "a.txt", proposedContent: "new" });
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(true);
  });

  it("accepts fenced json block", () => {
    const raw = "Here:\n```json\n{\"version\":1,\"targetPath\":\"a.txt\",\"proposedContent\":\"new\"}\n```";
    const res = parseAndValidatePatchResponse(raw, "a.txt", "old");
    expect(res.ok).toBe(true);
  });

  it("accepts snake_case aliases", () => {
    const json = JSON.stringify({ version: 1, target_path: "a.txt", proposed_content: "new" });
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(true);
  });
});

describe("patchContract — missing fields", () => {
  it("rejects missing targetPath", () => {
    const json = JSON.stringify({ version: 1, proposedContent: "new" });
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/Missing.*targetPath/i);
  });

  it("rejects missing proposedContent", () => {
    const json = JSON.stringify({ version: 1, targetPath: "a.txt" });
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/Missing.*proposedContent/i);
  });

  it("rejects missing version", () => {
    const json = JSON.stringify({ targetPath: "a.txt", proposedContent: "new" });
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/Missing.*version/i);
  });
});

describe("patchContract — version", () => {
  it("rejects unsupported version", () => {
    const json = JSON.stringify({ version: 2, targetPath: "a.txt", proposedContent: "new" });
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/Unsupported/);
  });

  it("rejects version as string", () => {
    const json = `{"version":"1","targetPath":"a.txt","proposedContent":"new"}`;
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(false);
  });
});

describe("patchContract — malformed", () => {
  it("rejects malformed JSON", () => {
    const res = parseAndValidatePatchResponse("not json", "a.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/Malformed|valid JSON/i);
  });

  it("rejects prose with no JSON", () => {
    const res = parseAndValidatePatchResponse("I will change the file but no json", "a.txt", "old");
    expect(res.ok).toBe(false);
  });

  it("extracts first JSON object from prose", () => {
    const raw = `Sure, here is the patch: {"version":1,"targetPath":"a.txt","proposedContent":"new"} and some trailing text`;
    const res = parseAndValidatePatchResponse(raw, "a.txt", "old");
    expect(res.ok).toBe(true);
  });
});

describe("patchContract — unexpected and multiple-file", () => {
  it("rejects unexpected fields", () => {
    const json = JSON.stringify({ version: 1, targetPath: "a.txt", proposedContent: "new", extra: "field" });
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/Unexpected/);
  });

  it("rejects multiple-file via files array", () => {
    const json = JSON.stringify({ version: 1, targetPath: "a.txt", proposedContent: "new", files: [{ path: "b.txt" }] });
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/Only single-file/);
  });

  it("rejects array response", () => {
    const json = JSON.stringify([{ version: 1, targetPath: "a.txt", proposedContent: "new" }]);
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/Only single-file|must be a JSON object/);
  });

  it("rejects multiple-file prose hint", () => {
    const json2 = JSON.stringify({ version: 1, targetPath: "a.txt", proposedContent: "new", patches: [] });
    const res = parseAndValidatePatchResponse(json2, "a.txt", "old");
    expect(res.ok).toBe(false);
  });
});

describe("patchContract — proposedContent", () => {
  it("rejects empty proposedContent", () => {
    const json = JSON.stringify({ version: 1, targetPath: "a.txt", proposedContent: "" });
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/Missing.*proposedContent/);
  });

  it("rejects oversized proposedContent", () => {
    const big = "a".repeat(300_000);
    const json = JSON.stringify({ version: 1, targetPath: "a.txt", proposedContent: big });
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/too large|character limit/i);
  });

  it("rejects binary proposedContent", () => {
    const json = JSON.stringify({ version: 1, targetPath: "a.txt", proposedContent: "hello\0world" });
    const res = parseAndValidatePatchResponse(json, "a.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/binary/i);
  });

  it("rejects no changes (equals original)", () => {
    const json = JSON.stringify({ version: 1, targetPath: "a.txt", proposedContent: "same" });
    const res = parseAndValidatePatchResponse(json, "a.txt", "same");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/No changes/);
  });
});

describe("patchContract — targetPath", () => {
  it("rejects absolute path", () => {
    const json = JSON.stringify({ version: 1, targetPath: "/etc/passwd", proposedContent: "new" });
    const res = parseAndValidatePatchResponse(json, "/etc/passwd", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/Absolute/);
  });

  it("rejects traversal", () => {
    const json = JSON.stringify({ version: 1, targetPath: "../outside.txt", proposedContent: "new" });
    const res = parseAndValidatePatchResponse(json, "../outside.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/traversal|Invalid/);
  });

  it("rejects sensitive .env", () => {
    const json = JSON.stringify({ version: 1, targetPath: ".env", proposedContent: "new" });
    const res = parseAndValidatePatchResponse(json, ".env", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/Sensitive/);
  });

  it("rejects sensitive .pem", () => {
    const json = JSON.stringify({ version: 1, targetPath: "certs/server.pem", proposedContent: "new" });
    const res = parseAndValidatePatchResponse(json, "certs/server.pem", "old");
    expect(res.ok).toBe(false);
  });

  it("rejects target mismatch", () => {
    const json = JSON.stringify({ version: 1, targetPath: "a.txt", proposedContent: "new" });
    const res = parseAndValidatePatchResponse(json, "b.txt", "old");
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/does not match/);
  });

  it("rejects empty targetPath", () => {
    const json = JSON.stringify({ version: 1, targetPath: "", proposedContent: "new" });
    const res = parseAndValidatePatchResponse(json, "", "old");
    expect(res.ok).toBe(false);
  });
});

describe("validateInstruction", () => {
  it("accepts valid instruction", () => {
    const res = validateInstruction("  fix bug  ");
    expect(res.ok).toBe(true);
    if (res.ok) expect(res.value).toBe("fix bug");
  });
  it("rejects empty", () => {
    expect(validateInstruction("   ").ok).toBe(false);
  });
  it("rejects too long", () => {
    expect(validateInstruction("a".repeat(3000)).ok).toBe(false);
  });
  it("rejects too short", () => {
    expect(validateInstruction("ab").ok).toBe(false);
  });
});
