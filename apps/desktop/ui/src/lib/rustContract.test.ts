import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * Cross-language contract check between the Rust core and the TypeScript UI.
 *
 * Everything else in the UI suite replaces the IPC boundary with a double, so a
 * renamed field, a changed enum tag, or a command argument spelled differently
 * on one side would pass those tests and fail only in the packaged app.
 *
 * **Limitation.** This parses source text. It does not execute serde or Tauri,
 * so it cannot prove how a value is encoded at runtime, and it covers only the
 * serde attributes it explicitly models. It is a regression guard against the
 * contract drifting, not a replacement for `cargo test` or a native run.
 */

const candidates = [
  resolve(process.cwd(), "src-tauri/src/inference"),
  resolve(process.cwd(), "apps/desktop/src-tauri/src/inference"),
];

const foundDir = candidates.find((candidate) => existsSync(join(candidate, "mod.rs")));

if (!foundDir) {
  throw new Error(
    `Rust inference sources not found; looked in ${candidates.join(", ")}`,
  );
}

const inferenceDir: string = foundDir;
// inferenceDir is <app>/src-tauri/src/inference, so three levels up is <app>.
const uiLibDir = resolve(inferenceDir, "../../../ui/src/lib");
const rustSrcDir = resolve(inferenceDir, "..");

const sources = {
  mod: readFileSync(join(inferenceDir, "mod.rs"), "utf8"),
  models: readFileSync(join(inferenceDir, "models.rs"), "utf8"),
  acquisition: readFileSync(join(inferenceDir, "acquisition.rs"), "utf8"),
  process: readFileSync(join(inferenceDir, "process.rs"), "utf8"),
  hardware: readFileSync(join(inferenceDir, "hardware.rs"), "utf8"),
  settings: readFileSync(join(inferenceDir, "settings.rs"), "utf8"),
  error: readFileSync(join(inferenceDir, "error.rs"), "utf8"),
  commands: readFileSync(join(inferenceDir, "commands.rs"), "utf8"),
  lib: readFileSync(join(rustSrcDir, "lib.rs"), "utf8"),
  exportMod: readFileSync(join(rustSrcDir, "export/mod.rs"), "utf8"),
  exportCommands: readFileSync(join(rustSrcDir, "export/commands.rs"), "utf8"),
  usageMod: readFileSync(join(rustSrcDir, "usage/mod.rs"), "utf8"),
  usageCommands: readFileSync(join(rustSrcDir, "usage/commands.rs"), "utf8"),
  historyStorage: readFileSync(join(rustSrcDir, "history/storage.rs"), "utf8"),
  historyCommands: readFileSync(join(rustSrcDir, "history/commands.rs"), "utf8"),
  authMod: readFileSync(join(rustSrcDir, "auth/mod.rs"), "utf8"),
};

const ipcSource = readFileSync(join(uiLibDir, "ipc.ts"), "utf8");
const inferenceSource = readFileSync(join(uiLibDir, "inference.ts"), "utf8");
const exportSource = readFileSync(join(uiLibDir, "export.ts"), "utf8");
const usageSource = readFileSync(join(uiLibDir, "usage.ts"), "utf8");
const historySource = readFileSync(join(uiLibDir, "history.ts"), "utf8");
const authSource = readFileSync(join(uiLibDir, "auth.ts"), "utf8");

function group(match: ArrayLike<string | undefined>, index: number): string {
  const value = match[index];

  if (value === undefined) {
    throw new Error(`Missing capture group ${index} in "${match[0] ?? ""}"`);
  }

  return value;
}

function camelCase(name: string): string {
  return name.replace(/_([a-z0-9])/g, (_, letter: string) => letter.toUpperCase());
}

function snakeCase(name: string): string {
  return name
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .toLowerCase()
    .replace(/^_/, "");
}

function sorted(values: readonly string[]): string[] {
  return [...values].sort();
}

// --------------------------------------------------------------------------
// Rust source shapes
// --------------------------------------------------------------------------

interface RustField {
  /** Field name as written in Rust. */
  rust: string;
  /** Field name on the wire, after serde renaming. */
  wire: string;
  /** Declared type, verbatim. */
  type: string;
  /** True when the field carries `#[serde(default)]`. */
  defaulted: boolean;
  /** True when the type is `Option<...>` and can be absent or null. */
  optional: boolean;
  /** True when a field-level `#[serde(rename = ...)]` was present. */
  renamed: boolean;
}

interface RustStruct {
  name: string;
  renameAll: string | null;
  /** True when the struct-level serde attribute contains `default`. */
  defaulted: boolean;
  fields: RustField[];
}

function structAttributes(source: string, name: string): string {
  const pattern = new RegExp(
    `((?:^[ \\t]*(?:///[^\\n]*|#\\[[^\\]]*\\])[ \\t]*\\n)*)` +
      `pub struct ${name} \\{`,
    "m",
  );
  const match = pattern.exec(source);

  if (!match) {
    throw new Error(`Rust struct ${name} not found`);
  }

  return group(match, 1);
}

function rustStruct(source: string, name: string): RustStruct {
  const attributes = structAttributes(source, name);
  const marker = `pub struct ${name} {`;
  const open = source.indexOf(marker);
  const close = source.indexOf("\n}", open);

  if (open < 0 || close < 0) {
    throw new Error(`Rust struct ${name} is not terminated`);
  }

  const serdeAttribute = /#\[serde\(([^\]]*)\)\]/.exec(attributes)?.[1] ?? "";
  const renameAll = /rename_all = "(\w+)"/.exec(serdeAttribute)?.[1] ?? null;

  const fields: RustField[] = [];
  let pendingDefault = false;
  let pendingRename: string | null = null;

  for (const line of source.slice(open + marker.length, close).split("\n")) {
    const trimmed = line.trim();

    if (trimmed.startsWith("///") || trimmed.startsWith("//")) {
      continue;
    }

    if (trimmed.startsWith("#[")) {
      pendingDefault = pendingDefault || /\bdefault\b/.test(trimmed);
      pendingRename = /rename = "([^"]+)"/.exec(trimmed)?.[1] ?? pendingRename;
      continue;
    }

    const field = /^pub (\w+):\s*(.+?),?$/.exec(trimmed);

    if (!field) {
      continue;
    }

    const rust = group(field, 1);
    const type = group(field, 2);

    fields.push({
      rust,
      wire: pendingRename ?? (renameAll === "camelCase" ? camelCase(rust) : rust),
      type,
      defaulted: pendingDefault,
      optional: type.startsWith("Option<"),
      renamed: pendingRename !== null,
    });

    pendingDefault = false;
    pendingRename = null;
  }

  return {
    name,
    renameAll,
    defaulted: /\bdefault\b/.test(serdeAttribute),
    fields,
  };
}

/** Body of a `pub enum`, found by counting braces (variants contain braces). */
function enumBody(source: string, name: string): string {
  const marker = `pub enum ${name} {`;
  const start = source.indexOf(marker);

  if (start < 0) {
    throw new Error(`Rust enum ${name} not found`);
  }

  let depth = 0;

  for (let index = start + marker.length - 1; index < source.length; index += 1) {
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

/** Attributes directly above an item, for tag and rename checks. */
function attributesBefore(source: string, marker: string): string {
  const start = source.indexOf(marker);

  if (start < 0) {
    throw new Error(`${marker} not found`);
  }

  return source.slice(Math.max(0, start - 240), start);
}

function variantNames(body: string): string[] {
  return [...body.matchAll(/^\s{4}([A-Z]\w*),?\s*$/gm)].map((match) => group(match, 1));
}

// --------------------------------------------------------------------------
// TypeScript shapes
// --------------------------------------------------------------------------

interface TypeScriptField {
  name: string;
  type: string;
  optional: boolean;
}

function interfaceFields(name: string): TypeScriptField[] {
  const marker = `export interface ${name} {`;
  const start = ipcSource.indexOf(marker);

  if (start < 0) {
    throw new Error(`TypeScript interface ${name} not found`);
  }

  const end = ipcSource.indexOf("\n}", start);

  if (end < 0) {
    throw new Error(`TypeScript interface ${name} is not terminated`);
  }

  return ipcSource
    .slice(start + marker.length, end)
    .split("\n")
    .map((line) => /^\s{2}(\w+)(\??):\s*(.+?);\s*$/.exec(line))
    .filter((match): match is RegExpExecArray => match !== null)
    .map((match) => ({
      name: group(match, 1),
      optional: group(match, 2) === "?",
      type: group(match, 3),
    }));
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

function interfaceUnion(name: string, field: string): string[] {
  const marker = `export interface ${name} {`;
  const start = ipcSource.indexOf(marker);
  const body = ipcSource.slice(start, ipcSource.indexOf("\n}", start));
  const match = new RegExp(`${field}:([^;]+);`).exec(body);

  if (!match) {
    throw new Error(`Field ${name}.${field} not found`);
  }

  return [...group(match, 1).matchAll(/"([a-z_]+)"/g)].map((member) => group(member, 1));
}

// --------------------------------------------------------------------------
// Fixtures: one table is the single source of truth for the wire contract
// --------------------------------------------------------------------------

interface StructFixture {
  /** Rust struct name. */
  rust: string;
  /** File the struct is declared in. */
  file: keyof typeof sources;
  /** TypeScript counterpart. */
  ts: string;
  /** Exact wire field names; a coordinated rename on both sides still fails. */
  wire: string[];
  /**
   * Request payloads are built by the UI, so an absent field is simply omitted
   * and the UI type is `field?: T`. Responses always carry the key, so a Rust
   * `Option<T>` becomes `field: T | null`. The two rules are not the same.
   */
  direction?: "request";
}

const STRUCTS: StructFixture[] = [
  {
    rust: "AppStatus",
    file: "mod",
    ts: "AppStatus",
    wire: ["name", "version", "engine", "modelPath", "runtimePath", "contextSize"],
  },
  { rust: "EngineStatus", file: "mod", ts: "EngineStatus", wire: ["state", "runtime", "model", "detail"] },
  { rust: "ChatTurn", file: "mod", ts: "ChatTurn", wire: ["role", "content"] },
  {
    rust: "GenerateRequest",
    file: "mod",
    ts: "GenerateRequestPayload",
    // `profileId` carries the profile a message was composed under, so the
    // core can refuse a stale tab instead of answering it with another model.
    wire: ["requestId", "messages", "maxTokens", "temperature", "profileId"],
    direction: "request",
  },
  {
    rust: "GenerationOutcome",
    file: "mod",
    ts: "GenerationOutcome",
    wire: ["requestId", "finishReason", "cancelled"],
  },
  {
    rust: "ModelInfo",
    file: "models",
    ts: "ModelInfo",
    wire: [
      "path",
      "fileName",
      "sizeBytes",
      "sizeGib",
      "ggufVersion",
      "tensorCount",
      "architecture",
      "name",
      "contextLength",
      "estimatedMinRamGib",
      "fitsAvailableRam",
      "warnings",
    ],
  },
  { rust: "RuntimeInfo", file: "process", ts: "RuntimeInfo", wire: ["path", "versionLine", "pinnedBuildMatched"] },
  {
    rust: "SystemInfo",
    file: "hardware",
    ts: "SystemInfo",
    wire: ["totalRamGib", "availableRamGib", "platform"],
  },
  {
    rust: "DownloadOutcome",
    file: "acquisition",
    ts: "ModelDownloadOutcome",
    wire: ["path", "fileName", "sizeBytes", "sha256", "cancelled"],
  },
  {
    rust: "IntegrityReport",
    file: "acquisition",
    ts: "IntegrityReport",
    wire: [
      "path",
      "fileName",
      "profileId",
      "sizeBytes",
      "sizeMatches",
      "sha256",
      "expectedSizeBytes",
      "expectedSha256",
      "digestMatches",
    ],
  },
  {
    rust: "Settings",
    file: "settings",
    ts: "Settings",
    wire: ["modelPath", "runtimePath", "contextSize", "modelId"],
  },
  { rust: "InferenceError", file: "error", ts: "InferenceError", wire: ["code", "message"] },
  { rust: "ExportCandidate", file: "exportMod", ts: "ExportCandidate", wire: ["path", "size"] },
  {
    rust: "ExportPlan",
    file: "exportMod",
    ts: "ExportPlan",
    wire: [
      "rootName",
      "files",
      "excludedSensitive",
      "excludedSymlink",
      "excludedUnsafe",
      "totalBytes",
      "truncated",
    ],
  },
  {
    rust: "ExportProgress",
    file: "exportMod",
    ts: "ExportProgress",
    wire: ["done", "total", "bytesDone", "bytesTotal", "current"],
  },
  {
    rust: "ExportOutcome",
    file: "exportMod",
    ts: "ExportOutcome",
    wire: ["path", "files", "bytes", "cancelled", "replaced"],
  },
  { rust: "ExportError", file: "exportMod", ts: "ExportError", wire: ["code", "message"] },
  {
    rust: "UsageStatus",
    file: "usageMod",
    ts: "UsageStatus",
    wire: [
      "allowance",
      "used",
      "remaining",
      "day",
      "resetAtMs",
      "policyId",
      "provisional",
      "exhausted",
      "notice",
      "error",
    ],
  },
  {
    rust: "SingleExportRequest",
    file: "exportMod",
    ts: "SingleExportRequest",
    wire: ["relativePath", "destination", "overwrite"],
    direction: "request",
  },
  {
    rust: "ZipExportRequest",
    file: "exportMod",
    ts: "ZipExportRequest",
    wire: ["selection", "destination", "overwrite"],
    direction: "request",
  },
];

describe("serialised structs match the TypeScript interfaces", () => {
  for (const fixture of STRUCTS) {
    it(`${fixture.rust} ⇄ ${fixture.ts}`, () => {
      const backend = rustStruct(sources[fixture.file], fixture.rust);
      const frontend = interfaceFields(fixture.ts);

      expect(backend.fields.length).toBeGreaterThan(0);
      expect(frontend.length).toBeGreaterThan(0);

      // Wire names, exactly — the pinned list catches a rename on both sides.
      expect(sorted(backend.fields.map((field) => field.wire))).toEqual(sorted(fixture.wire));
      expect(sorted(frontend.map((field) => field.name))).toEqual(sorted(fixture.wire));

      if (fixture.direction === "request") {
        // Covered by the dedicated optional-field test below.
        return;
      }

      // Nullability: an `Option<T>` in a response serialises as `null`.
      for (const field of backend.fields) {
        const counterpart = frontend.find((candidate) => candidate.name === field.wire);

        if (!counterpart) {
          throw new Error(`${fixture.ts}.${field.wire} is missing`);
        }

        expect({
          field: field.wire,
          nullable: counterpart.type.includes("null"),
        }).toEqual({ field: field.wire, nullable: field.optional });
      }
    });
  }

  it("marks exactly the optional request fields optional in TypeScript", () => {
    const backend = rustStruct(sources.mod, "GenerateRequest");
    const frontend = interfaceFields("GenerateRequestPayload");

    // `maxTokens` and `temperature` may be omitted; serde needs `default` for
    // that to deserialise, so the attribute and the `?` must travel together.
    for (const field of backend.fields) {
      const counterpart = frontend.find((candidate) => candidate.name === field.wire);

      expect({ field: field.wire, optional: counterpart?.optional }).toEqual({
        field: field.wire,
        optional: field.defaulted && field.optional,
      });
    }
  });

  it("keeps Settings loadable from a partial file", () => {
    const settings = rustStruct(sources.settings, "Settings");

    // A hand-edited or older settings file must not break startup.
    expect(settings.defaulted).toBe(true);
    expect(settings.renameAll).toBe("camelCase");
  });
});

describe("enumerations match the TypeScript unions", () => {
  it("serialises EngineState variants as the UI spells them", () => {
    const attributes = attributesBefore(sources.mod, "pub enum EngineState {");

    expect(attributes).toContain('rename_all = "snake_case"');

    const variants = variantNames(enumBody(sources.mod, "EngineState"));
    const serialised = variants.map((variant) => snakeCase(variant));

    expect(sorted(serialised)).toEqual([
      "error",
      "generating",
      "loading",
      "not_configured",
      "ready",
      "stopped",
    ]);
    expect(sorted(serialised)).toEqual(sorted(typeUnion("EngineState")));
  });

  it("reaches every EngineState variant from the Rust core", () => {
    const defined = variantNames(enumBody(sources.mod, "EngineState"));
    const used = [...sources.mod.matchAll(/EngineState::(\w+)/g)].map((match) =>
      group(match, 1),
    );

    // A variant that is never constructed is dead; one constructed but not
    // declared would not compile. Both are catchable without a compiler.
    expect(sorted([...new Set(used)])).toEqual(sorted(defined));
  });

  it("streams GenerationEvent with the tag and payload the UI reads", () => {
    const attributes = attributesBefore(sources.mod, "pub enum GenerationEvent {");

    expect(attributes).toContain('tag = "type"');
    expect(attributes).toContain('rename_all = "snake_case"');

    const body = enumBody(sources.mod, "GenerationEvent");
    const variants = [...body.matchAll(/^\s{4}([A-Z]\w*)\s*\{/gm)].map((match) =>
      group(match, 1).toLowerCase(),
    );

    expect(sorted(variants)).toEqual(["delta", "started"]);
    expect(sorted(variants)).toEqual(sorted(interfaceUnion("GenerationEvent", "type")));

    // `type` is the serde tag rather than a Rust field, so compare the rest.
    const payloadFields = [
      ...new Set(
        [...body.matchAll(/^\s{8}(\w+):/gm)].map((match) => camelCase(group(match, 1))),
      ),
    ];

    expect(sorted(payloadFields)).toEqual(
      sorted(
        interfaceFields("GenerationEvent")
          .map((field) => field.name)
          .filter((name) => name !== "type"),
      ),
    );

    // Every variant must echo the request id, or the UI cannot drop stale
    // deltas; both variants also rename it explicitly.
    expect([...body.matchAll(/#\[serde\(rename = "requestId"\)\]/g)]).toHaveLength(
      variants.length,
    );
  });
});

describe("the Rust core is internally consistent", () => {
  it("only calls InferenceError constructors that exist", () => {
    const defined = new Set(
      [...sources.error.matchAll(/pub fn (\w+)\(/g)].map((match) => group(match, 1)),
    );

    const referenced = new Set<string>();

    for (const source of Object.values(sources)) {
      for (const match of source.matchAll(/InferenceError::(\w+)\(/g)) {
        referenced.add(group(match, 1));
      }
    }

    expect(referenced.size).toBeGreaterThan(0);

    const missing = [...referenced].filter((name) => !defined.has(name));

    // A typo here is exactly what `cargo check` would catch first.
    expect(missing).toEqual([]);
  });
});

// --------------------------------------------------------------------------
// Tauri command surface
// --------------------------------------------------------------------------

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

  const combined = `${sources.commands}\n${sources.exportCommands}\n${sources.usageCommands}`;

  return [...combined.matchAll(pattern)].map((match) => {
    const args = splitTopLevel(group(match, 3))
      .map((parameter) => /^(\w+):\s*([\s\S]+)$/.exec(parameter))
      .filter((parsed): parsed is RegExpExecArray => parsed !== null)
      // Tauri injects these from the runtime, so the UI never sends them.
      .filter((parsed) => {
        const type = group(parsed, 2);

        return !type.startsWith("State<") && type !== "AppHandle";
      })
      .map((parsed) => camelCase(group(parsed, 1)));

    return { attribute: group(match, 1), name: group(match, 2), args };
  });
}

function registeredCommands(): string[] {
  const match = /tauri::generate_handler!\[([\s\S]*?)\]/.exec(sources.lib);

  if (!match) {
    throw new Error("generate_handler![...] not found in lib.rs");
  }

  return [...group(match, 1).matchAll(/(?:inference|export|usage)::commands::(\w+)/g)].map(
    (command) => group(command, 1),
  );
}

/** Command name → argument keys the UI sends, from every call/invoke site. */
function uiCommands(): Map<string, string[]> {
  const calls = new Map<string, string[]>();
  const pattern = /\b(?:call|invoke)<\w+>\("(\w+)"(?:,\s*\{([\s\S]*?)\})?\)/g;

  for (const match of `${inferenceSource}\n${exportSource}\n${usageSource}`.matchAll(pattern)) {
    const body = match[2];
    const keys =
      body === undefined
        ? []
        : splitTopLevel(body)
            .map((argument) => argument.split(":")[0]?.trim() ?? argument)
            .filter((key) => /^\w+$/.test(key));

    calls.set(group(match, 1), keys);
  }

  return calls;
}

describe("Tauri command surface", () => {
  const commands = rustCommands();

  it("declares commands that are unique and registered exactly once", () => {
    expect(commands.length).toBeGreaterThan(0);

    const defined = commands.map((command) => command.name);
    const registered = registeredCommands();

    expect(sorted(defined)).toEqual(sorted([...new Set(defined)]));
    expect(sorted(registered)).toEqual(sorted([...new Set(registered)]));
    expect(sorted(registered)).toEqual(sorted(defined));
  });

  it("keeps the default camelCase argument conversion the UI relies on", () => {
    for (const command of commands) {
      expect({ command: command.name, attribute: command.attribute }).toEqual({
        command: command.name,
        attribute: "tauri::command",
      });
    }
  });

  it("invokes every registered command and nothing else", () => {
    expect(sorted([...uiCommands().keys()])).toEqual(
      sorted(commands.map((command) => command.name)),
    );
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

  it("agrees about which commands take no arguments at all", () => {
    const calls = uiCommands();
    const argless = commands
      .filter((command) => command.args.length === 0)
      .map((command) => command.name);

    // These read state the core already owns; Tauri injects `State`/`AppHandle`
    // rather than the UI sending them. Adding an argument to one of these Rust
    // signatures without updating the wrapper would break every call.
    expect(sorted(argless)).toEqual([
      "cancel_export",
      "cancel_model_download",
      "check_model_readiness",
      "get_app_status",
      "get_model_catalog",
      "get_model_profile",
      "get_settings",
      "get_system_info",
      "get_usage_status",
      "load_model",
      "unload_model",
    ]);

    for (const name of argless) {
      expect({ name, args: calls.get(name) }).toEqual({ name, args: [] });
    }
  });

  it("pins the argument names of the stateful commands", () => {
    const byName = new Map(commands.map((command) => [command.name, sorted(command.args)]));

    expect(byName.get("start_generation")).toEqual(["onEvent", "request"]);
    expect(byName.get("cancel_generation")).toEqual(["requestId"]);
    expect(byName.get("validate_model")).toEqual(["contextSize", "path"]);
    expect(byName.get("save_settings")).toEqual(["settings"]);
    expect(byName.get("start_model_download")).toEqual(["onEvent", "profileId", "targetDir"]);
    expect(byName.get("verify_model")).toEqual(["path", "profileId"]);
    expect(byName.get("get_disk_space")).toEqual(["targetDir"]);
    expect(byName.get("select_model")).toEqual(["id"]);
    expect(byName.get("plan_workspace_export")).toEqual(["selection"]);
    expect(byName.get("suggest_export_path")).toEqual(["kind", "relativePath"]);
    expect(byName.get("export_single_file")).toEqual(["onEvent", "request"]);
    expect(byName.get("export_zip")).toEqual(["onEvent", "request"]);
  });
});

// --------------------------------------------------------------------------
// Phase 7: the daily allowance must be the same number on both sides
// --------------------------------------------------------------------------

describe("daily usage policy", () => {
  it("publishes the allowance the backend enforces", () => {
    const constant = /pub const DAILY_ALLOWANCE: u32 = (\d+);/.exec(sources.usageMod);
    const published = /PUBLISHED_DAILY_ALLOWANCE = (\d+);/.exec(usageSource);

    expect(constant).not.toBeNull();
    expect(published).not.toBeNull();
    expect(group(published as RegExpExecArray, 1)).toBe(group(constant as RegExpExecArray, 1));
  });

  it("marks the allowance as provisional in the core", () => {
    expect(sources.usageMod).toContain("pub const ALLOWANCE_IS_PROVISIONAL: bool = true;");
  });

  it("enforces the allowance inside the only command that can start a model", () => {
    // Every `generate()` in the UI funnels through `start_generation`; the
    // allowance is reserved there, before the engine is asked to work.
    expect(sources.commands).toContain("usage.reserve(");
    expect(sources.commands).toContain("charge.commit()");
    expect(sources.commands).toContain("if accepted.load(");
  });

  it("has exactly one caller of the streaming client in the whole core", () => {
    // A second entry point that posted to the engine directly would bypass the
    // allowance. The client module defines `stream_chat`; only the generation
    // command may call it.
    const files = ["inference/client.rs", "inference/commands.rs", "inference/mod.rs"];
    const callers = files.filter((file) => {
      const source = readFileSync(join(rustSrcDir, file), "utf8");
      return (source.match(/\bstream_chat\(/g) ?? []).length > 0;
    });

    const client = readFileSync(join(rustSrcDir, "inference/client.rs"), "utf8");
    const command = readFileSync(join(rustSrcDir, "inference/commands.rs"), "utf8");

    expect(callers.sort()).toEqual(["inference/client.rs", "inference/commands.rs"]);
    expect((client.match(/\bstream_chat\(/g) ?? []).length).toBe(1);
    expect((command.match(/\bstream_chat\(/g) ?? []).length).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// Whole-surface IPC audit (Phase 8)
// ---------------------------------------------------------------------------
//
// The checks above model a subset of commands from the modules that carry a
// hand-written DTO. They cannot notice a `call("typo")` in, say, the history or
// workspace wrapper, because those files were never read. This audit walks
// every non-test UI source file instead, and cross-checks the result against
// the one list that actually decides what exists at runtime:
// `tauri::generate_handler![...]`.
//
// It is still a source-text check: it proves the names line up, not that the
// arguments or return types do.

const uiSrcDir = resolve(uiLibDir, "..");

function uiSourceFiles(dir: string): string[] {
  const files: string[] = [];

  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);

    if (entry.isDirectory()) {
      files.push(...uiSourceFiles(path));
    } else if (/\.tsx?$/.test(entry.name) && !/\.test\./.test(entry.name)) {
      files.push(path);
    }
  }

  return files;
}

function handlerCommands(): Set<string> {
  const handler = /tauri::generate_handler!\[([\s\S]*?)\]/.exec(sources.lib);

  if (!handler?.[1]) {
    throw new Error("generate_handler![...] not found in lib.rs");
  }

  return new Set(
    handler[1]
      .split(",")
      .map((entry) => entry.trim().split("::").pop() ?? "")
      .filter((name) => /^\w+$/.test(name)),
  );
}

function invokedCommands(): Map<string, string[]> {
  const pattern = /\b(?:call|invoke)\s*(?:<[^()]*?>)?\s*\(\s*["']([A-Za-z0-9_]+)["']/g;
  const found = new Map<string, string[]>();

  for (const file of uiSourceFiles(uiSrcDir)) {
    const text = readFileSync(file, "utf8");

    for (const match of text.matchAll(pattern)) {
      const name = match[1];

      if (!name) {
        continue;
      }

      const list = found.get(name) ?? [];
      list.push(file.replace(uiSrcDir, ""));
      found.set(name, list);
    }
  }

  return found;
}

describe("IPC command surface", () => {
  it("every command the UI invokes exists in generate_handler!", () => {
    const registered = handlerCommands();
    const missing = [...invokedCommands().keys()].filter((name) => !registered.has(name));

    expect(missing).toEqual([]);
  });

  it("every registered command is reachable from the UI", () => {
    const invoked = invokedCommands();
    const orphaned = [...handlerCommands()].filter((name) => !invoked.has(name));

    expect(orphaned).toEqual([]);
  });
});

function authInterfaceWireFields(name: string): string[] {
  const marker = `export interface ${name} {`;
  const start = authSource.indexOf(marker);
  if (start < 0) {
    throw new Error(`TypeScript auth interface ${name} not found`);
  }
  const end = authSource.indexOf("\n}", start);
  return authSource
    .slice(start + marker.length, end)
    .split("\n")
    .map((line) => /^\s{2}(\w+)\??:\s*.+?;\s*$/.exec(line))
    .filter((match): match is RegExpExecArray => match !== null)
    .map((match) => group(match, 1));
}

function authRustWireFields(name: string): string[] {
  const marker = `pub struct ${name} {`;
  const start = sources.authMod.indexOf(marker);
  if (start < 0) {
    throw new Error(`Rust auth struct ${name} not found`);
  }
  const end = sources.authMod.indexOf("\n}", start);
  return sources.authMod
    .slice(start + marker.length, end)
    .split("\n")
    .map((line) => /^\s*pub (\w+):/.exec(line))
    .filter((match): match is RegExpExecArray => match !== null)
    .map((match) => camelCase(group(match, 1)));
}

describe("Auth, session, and diagnostics wire contract", () => {
  const AUTH_DTO_SHAPES: Array<[string, string[]]> = [
    ["AuthUser", ["id", "name", "email", "emailVerified"]],
    [
      "AccountProfile",
      [
        "id",
        "name",
        "email",
        "emailVerified",
        "hasPassword",
        "providers",
        "planId",
        "planName",
        "createdAt",
      ],
    ],
    ["AuthSessionInfo", ["user", "authenticated"]],
    ["AuthStatusResponse", ["status", "message", "emailDelivery"]],
    ["AuthError", ["code", "message", "retryAfterSeconds"]],
    [
      "DeveloperDiagnostics",
      [
        "platform",
        "arch",
        "appVersion",
        "credentialVault",
        "authServiceUrl",
        "cspEnforced",
        "offlineCapable",
      ],
    ],
    ["LoginPayload", ["email", "password"]],
    ["RegisterPayload", ["name", "email", "password"]],
    ["VerifyEmailPayload", ["token"]],
    ["ResendVerificationPayload", ["email"]],
    ["RecoveryPayload", ["email"]],
    ["ResetPasswordPayload", ["token", "password"]],
    ["ChangePasswordPayload", ["currentPassword", "newPassword"]],
    ["UpdateProfilePayload", ["name"]],
    ["DeleteAccountPayload", ["confirmEmail", "currentPassword"]],
  ];

  for (const [dtoName, expectedFields] of AUTH_DTO_SHAPES) {
    it(`keeps ${dtoName} aligned between Rust and TypeScript`, () => {
      expect(sorted(authRustWireFields(dtoName))).toEqual(sorted(expectedFields));
      expect(sorted(authInterfaceWireFields(dtoName))).toEqual(sorted(expectedFields));
    });
  }
});

describe("Conversation history wire contract", () => {
  function historyTsFields(name: string): string[] {
    const marker = `export interface ${name} {`;
    const start = historySource.indexOf(marker);
    if (start < 0) throw new Error(`TypeScript history interface ${name} not found`);
    const end = historySource.indexOf("\n}", start);
    return historySource
      .slice(start + marker.length, end)
      .split("\n")
      .map((line) => /^\s{2}(\w+)\??:\s*.+?;\s*$/.exec(line))
      .filter((match): match is RegExpExecArray => match !== null)
      .map((match) => group(match, 1));
  }

  it("keeps StoredMessage / HistoryMessage aligned between Rust and TypeScript", () => {
    expect(sorted(rustStruct(sources.historyStorage, "StoredMessage").fields.map((f) => f.wire)))
      .toEqual(sorted(historyTsFields("HistoryMessage")));
  });

  it("keeps StoredConversation / PersistedConversation aligned between Rust and TypeScript", () => {
    expect(
      sorted(rustStruct(sources.historyStorage, "StoredConversation").fields.map((f) => f.wire)),
    ).toEqual(sorted(historyTsFields("PersistedConversation")));
  });

  it("keeps ConversationSummary aligned between Rust and TypeScript", () => {
    expect(
      sorted(rustStruct(sources.historyStorage, "ConversationSummary").fields.map((f) => f.wire)),
    ).toEqual(sorted(historyTsFields("ConversationSummary")));
  });

  it("keeps HistoryError aligned between Rust and TypeScript", () => {
    expect(
      sorted(rustStruct(sources.historyStorage, "HistoryError").fields.map((f) => f.wire)),
    ).toEqual(sorted(historyTsFields("HistoryError")));
  });
});
