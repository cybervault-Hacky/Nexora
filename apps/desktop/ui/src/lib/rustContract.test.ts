import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Cross-language contract check.
 *
 * The Rust core serialises structs with serde; the UI consumes them through
 * `lib/ipc.ts`. Every other UI test replaces the IPC boundary with a double, so
 * a camelCase/snake_case mismatch or a renamed field would pass those tests and
 * fail only at runtime, in the packaged app, on Windows. Nothing here executes
 * Rust or Tauri: it parses the real source files and compares the serialised
 * names with the TypeScript declarations.
 *
 * The Rust backend is not compiled in this environment (no toolchain), so this
 * is the strongest cross-language check available here. It is not a substitute
 * for `cargo test` or a native run.
 */

const candidates = [
  resolve(process.cwd(), "src-tauri/src/inference"),
  resolve(process.cwd(), "apps/desktop/src-tauri/src/inference"),
];

const foundDir = candidates.find((candidate) =>
  existsSync(join(candidate, "mod.rs")),
);

if (!foundDir) {
  throw new Error(
    `Rust inference sources not found; looked in ${candidates.join(", ")}`,
  );
}

const inferenceDir: string = foundDir;

// inferenceDir is <app>/src-tauri/src/inference, so three levels up is <app>.
const uiLibDir = resolve(inferenceDir, "../../../ui/src/lib");
const ipcSource = readFileSync(resolve(uiLibDir, "ipc.ts"), "utf8");
const inferenceSource = readFileSync(resolve(uiLibDir, "inference.ts"), "utf8");
const libSource = readFileSync(resolve(inferenceDir, "../lib.rs"), "utf8");

function readRust(file: string): string {
  return readFileSync(join(inferenceDir, file), "utf8");
}

function group(match: ArrayLike<string | undefined>, index: number): string {
  const value = match[index];

  if (value === undefined) {
    throw new Error(`Missing capture group ${index} in "${match[0] ?? ""}"`);
  }

  return value;
}

function camelCase(name: string): string {
  return name.replace(/_([a-z0-9])/g, (_, letter: string) =>
    letter.toUpperCase(),
  );
}

function snakeCase(name: string): string {
  return name
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .toLowerCase()
    .replace(/^_/, "");
}

function interfaceBody(name: string): string {
  const marker = `export interface ${name} {`;
  const start = ipcSource.indexOf(marker);

  if (start < 0) {
    throw new Error(`TypeScript interface ${name} not found`);
  }

  const end = ipcSource.indexOf("\n}", start);

  if (end < 0) {
    throw new Error(`TypeScript interface ${name} is not terminated`);
  }

  return ipcSource.slice(start + marker.length, end);
}

function interfaceFields(name: string): string[] {
  return [...interfaceBody(name).matchAll(/^\s{2}(\w+)\??:/gm)].map((match) =>
    group(match, 1),
  );
}

/** Quoted members of a union-typed interface field, e.g. `state: "a" | "b"`. */
function interfaceUnion(name: string, field: string): string[] {
  const match = new RegExp(`${field}:([^;]+);`).exec(interfaceBody(name));

  if (!match) {
    throw new Error(`Field ${name}.${field} not found`);
  }

  return [...group(match, 1).matchAll(/"([a-z_]+)"/g)].map((member) =>
    group(member, 1),
  );
}

function typeUnion(name: string): string[] {
  const marker = `export type ${name} =`;
  const start = ipcSource.indexOf(marker);

  if (start < 0) {
    throw new Error(`TypeScript union ${name} not found`);
  }

  const end = ipcSource.indexOf(";", start);
  const body = ipcSource.slice(start + marker.length, end);

  return [...body.matchAll(/"([a-z_]+)"/g)].map((match) => group(match, 1));
}

interface RustStruct {
  fields: string[];
  renameAll: string | null;
}

function rustStruct(source: string, name: string): RustStruct {
  const pattern = new RegExp(
    `((?:^[ \\t]*(?:///[^\\n]*|#\\[[^\\]]*\\])[ \\t]*\\n)*)` +
      `pub struct ${name} \\{`,
    "m",
  );
  const match = pattern.exec(source);

  if (!match) {
    throw new Error(`Rust struct ${name} not found`);
  }

  const open = match.index + match[0].length - 1;
  const close = source.indexOf("\n}", open);

  if (close < 0) {
    throw new Error(`Rust struct ${name} is not terminated`);
  }

  const renameAll = /rename_all = "(\w+)"/.exec(group(match, 1))?.[1] ?? null;
  const fields: string[] = [];
  let pendingRename: string | null = null;

  for (const line of source.slice(open + 1, close).split("\n")) {
    const trimmed = line.trim();
    const fieldRename = /^#\[serde\(rename = "([^"]+)"\)\]$/.exec(trimmed);

    if (fieldRename) {
      pendingRename = group(fieldRename, 1);
      continue;
    }

    if (trimmed.startsWith("#[")) {
      continue;
    }

    const field = /^pub (\w+):/.exec(trimmed);

    if (field) {
      const name = group(field, 1);
      fields.push(
        pendingRename ?? (renameAll === "camelCase" ? camelCase(name) : name),
      );
      pendingRename = null;
    }
  }

  return { fields, renameAll };
}

/** Body of a `pub enum`, found by counting braces (variants contain braces). */
function enumBody(source: string, name: string): string {
  const marker = `pub enum ${name} {`;
  const start = source.indexOf(marker);

  if (start < 0) {
    throw new Error(`Rust enum ${name} not found`);
  }

  let depth = 0;

  for (
    let index = start + marker.length - 1;
    index < source.length;
    index += 1
  ) {
    const character = source[index];

    if (character === "{") {
      depth += 1;
    } else if (character === "}") {
      depth -= 1;

      if (depth === 0) {
        return source.slice(start + marker.length, index);
      }
    }
  }

  throw new Error(`Rust enum ${name} is not terminated`);
}

function sorted(values: readonly string[]): string[] {
  return [...values].sort();
}

describe("Rust serialisation matches the TypeScript contract", () => {
  const structs: Array<{ rust: string; file: string; ts: string }> = [
    { rust: "AppStatus", file: "mod.rs", ts: "AppStatus" },
    { rust: "EngineStatus", file: "mod.rs", ts: "EngineStatus" },
    { rust: "ChatTurn", file: "mod.rs", ts: "ChatTurn" },
    { rust: "GenerateRequest", file: "mod.rs", ts: "GenerateRequestPayload" },
    { rust: "GenerationOutcome", file: "mod.rs", ts: "GenerationOutcome" },
    { rust: "ModelInfo", file: "models.rs", ts: "ModelInfo" },
    { rust: "RuntimeInfo", file: "process.rs", ts: "RuntimeInfo" },
    { rust: "SystemInfo", file: "hardware.rs", ts: "SystemInfo" },
    { rust: "Settings", file: "settings.rs", ts: "Settings" },
    { rust: "InferenceError", file: "error.rs", ts: "InferenceError" },
  ];

  for (const { rust, file, ts } of structs) {
    it(`serialises ${rust} exactly as ${ts} expects`, () => {
      const backend = rustStruct(readRust(file), rust).fields;
      const frontend = interfaceFields(ts);

      expect(backend.length).toBeGreaterThan(0);
      expect(frontend.length).toBeGreaterThan(0);
      expect(sorted(backend)).toEqual(sorted(frontend));
    });
  }

  it("serialises every EngineState variant as the UI union spells it", () => {
    const source = readRust("mod.rs");
    const start = source.indexOf("pub enum EngineState {");

    expect(source.slice(Math.max(0, start - 200), start)).toContain(
      'rename_all = "snake_case"',
    );

    const body = enumBody(source, "EngineState");
    const variants = [...body.matchAll(/^\s{4}([A-Z]\w*),?\s*$/gm)].map(
      (match) => snakeCase(group(match, 1)),
    );

    expect(variants.length).toBeGreaterThan(0);
    expect(sorted(variants)).toEqual(sorted(typeUnion("EngineState")));
  });

  it("pins the wire shapes the UI was built against", () => {
    const shapes: Record<string, string[]> = {
      AppStatus: ["contextSize", "engine", "modelPath", "name", "runtimePath", "version"],
      GenerateRequest: ["maxTokens", "messages", "requestId", "temperature"],
      GenerationOutcome: ["cancelled", "finishReason", "requestId"],
      GenerationEvent: ["requestId", "text"],
      EngineState: ["error", "generating", "loading", "not_configured", "ready", "stopped"],
    };

    for (const [name, expected] of Object.entries(shapes)) {
      if (name === "GenerationEvent") {
        const body = enumBody(readRust("mod.rs"), name);
        const fields = [
          ...new Set(
            [...body.matchAll(/^\s{8}(\w+):/gm)].map((match) =>
              camelCase(group(match, 1)),
            ),
          ),
        ];

        expect(sorted(fields)).toEqual(sorted(expected));
      } else if (name === "EngineState") {
        const body = enumBody(readRust("mod.rs"), name);
        const variants = [...body.matchAll(/^\s{4}([A-Z]\w*),?\s*$/gm)].map(
          (match) => snakeCase(group(match, 1)),
        );

        expect(sorted(variants)).toEqual(sorted(expected));
      } else {
        expect(
          sorted(rustStruct(readRust("mod.rs"), name).fields),
        ).toEqual(sorted(expected));
      }
    }
  });

  it("streams GenerationEvent payloads with the tag and fields the UI reads", () => {
    const source = readRust("mod.rs");
    const start = source.indexOf("pub enum GenerationEvent {");
    const attributes = source.slice(Math.max(0, start - 200), start);

    expect(attributes).toContain('tag = "type"');
    expect(attributes).toContain('rename_all = "snake_case"');

    const body = enumBody(source, "GenerationEvent");
    const variants = [...body.matchAll(/^\s{4}([A-Z]\w*)\s*\{/gm)].map(
      (match) => group(match, 1).toLowerCase(),
    );

    expect(sorted(variants)).toEqual(["delta", "started"]);
    expect(sorted(variants)).toEqual(
      sorted(interfaceUnion("GenerationEvent", "type")),
    );

    // `type` is the serde tag, not a Rust field, so compare the rest.
    // Both variants carry `request_id`, but the UI sees one field name.
    const fields = [
      ...new Set(
        [...body.matchAll(/^\s{8}(\w+):/gm)].map((match) =>
          camelCase(group(match, 1)),
        ),
      ),
    ];
    const expected = interfaceFields("GenerationEvent").filter(
      (field) => field !== "type",
    );

    expect(sorted(fields)).toEqual(sorted(expected));
  });
});

interface RustCommand {
  attribute: string;
  name: string;
  args: string[];
}

/** Splits `a: X<A, B>, b: Y` on commas that are not inside generics. */
function splitTopLevel(text: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let current = "";

  for (const character of text) {
    if (character === "<") {
      depth += 1;
    } else if (character === ">") {
      depth -= 1;
    }

    if (character === "," && depth === 0) {
      parts.push(current);
      current = "";
    } else {
      current += character;
    }
  }

  parts.push(current);

  return parts.map((part) => part.trim()).filter(Boolean);
}

function rustCommands(): RustCommand[] {
  const pattern =
    /^#\[(tauri::command[^\]]*)\]\s*\n(?:pub )?(?:async )?fn (\w+)\(([\s\S]*?)\)\s*->/gm;

  return [...readRust("commands.rs").matchAll(pattern)].map((match) => {
    const args = splitTopLevel(group(match, 3))
      .map((parameter) => /^(\w+):\s*([\s\S]+)$/.exec(parameter))
      .filter((parsed): parsed is RegExpExecArray => parsed !== null)
      .filter((parsed) => {
        const type = group(parsed, 2);

        return !type.startsWith("State<") && type !== "AppHandle";
      })
      .map((parsed) => camelCase(group(parsed, 1)));

    return { attribute: group(match, 1), name: group(match, 2), args };
  });
}

function registeredCommands(): string[] {
  const match = /tauri::generate_handler!\[([\s\S]*?)\]/.exec(libSource);

  if (!match) {
    throw new Error("generate_handler![...] not found in lib.rs");
  }

  return [...group(match, 1).matchAll(/inference::commands::(\w+)/g)].map(
    (command) => group(command, 1),
  );
}

/** Command name → argument keys the UI sends, from every call/invoke site. */
function uiCommands(): Map<string, string[]> {
  const calls = new Map<string, string[]>();

  const record = (name: string, body: string | undefined) => {
    const keys =
      body === undefined
        ? []
        : splitTopLevel(body)
            .map((argument) => argument.split(":")[0]?.trim() ?? argument)
            .filter((key) => /^\w+$/.test(key));

    calls.set(name, keys);
  };

  const pattern =
    /\b(?:call|invoke)<\w+>\("(\w+)"(?:,\s*\{([\s\S]*?)\})?\)/g;

  for (const match of inferenceSource.matchAll(pattern)) {
    record(group(match, 1), match[2]);
  }

  return calls;
}

describe("Tauri command surface", () => {
  const commands = rustCommands();

  it("registers each command once, and only real commands", () => {
    expect(commands.length).toBeGreaterThan(0);

    const defined = commands.map((command) => command.name);
    const registered = registeredCommands();

    expect(sorted(defined)).toEqual(sorted([...new Set(defined)]));
    expect(sorted(registered)).toEqual(sorted([...new Set(registered)]));
    expect(sorted(registered)).toEqual(sorted(defined));
  });

  it("keeps the default camelCase argument conversion, which the UI assumes", () => {
    for (const command of commands) {
      expect(command.attribute).toBe("tauri::command");
    }
  });

  it("invokes every registered command and nothing else", () => {
    const calls = uiCommands();

    expect(sorted([...calls.keys()])).toEqual(
      sorted(commands.map((command) => command.name)),
    );
  });

  it("pins the argument names of the streaming commands", () => {
    const byName = new Map(
      commands.map((command) => [command.name, sorted(command.args)]),
    );

    expect(byName.get("start_generation")).toEqual(["onEvent", "request"]);
    expect(byName.get("cancel_generation")).toEqual(["requestId"]);
    expect(byName.get("validate_model")).toEqual(["contextSize", "path"]);
  });

  it("sends each argument under the key Tauri deserialises", () => {
    const calls = uiCommands();

    for (const command of commands) {
      const sent = calls.get(command.name);

      if (sent === undefined) {
        throw new Error(`${command.name} is never invoked by the UI`);
      }

      expect({ command: command.name, args: sorted(sent) }).toEqual({
        command: command.name,
        args: sorted(command.args),
      });
    }
  });
});
