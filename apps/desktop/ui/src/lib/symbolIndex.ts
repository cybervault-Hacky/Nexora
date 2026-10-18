import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell } from "./ipc";

export type SymbolKind =
  | "function"
  | "method"
  | "class"
  | "interface"
  | "type"
  | "struct"
  | "enum"
  | "trait"
  | "constant"
  | "import"
  | "export"
  | "module"
  | "file";

export interface Symbol {
  name: string;
  kind: SymbolKind;
  path: string;
  startLine: number;
  endLine: number;
  detail?: string;
}

export interface SymbolIndex {
  version: number;
  root: string;
  symbols: Symbol[];
  fileCount: number;
  truncated: boolean;
  unsupported: number;
  parseErrors: number;
  generatedAt: number;
}

function toError(cause: unknown): { code: string; message: string } {
  if (typeof cause === "object" && cause !== null && "code" in cause && "message" in cause) {
    return cause as { code: string; message: string };
  }
  return { code: "ipc", message: cause instanceof Error ? cause.message : String(cause) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw toError(cause);
  }
}

export async function getSymbolIndex(root: string): Promise<SymbolIndex> {
  if (!isDesktopShell()) {
    throw toError({ code: "desktop_only", message: "Symbol index is only available in the desktop app." });
  }
  return call<SymbolIndex>("get_symbol_index", { root });
}

export async function refreshSymbolIndex(root: string): Promise<SymbolIndex> {
  if (!isDesktopShell()) {
    throw toError({ code: "desktop_only", message: "Symbol index is only available in the desktop app." });
  }
  return call<SymbolIndex>("refresh_symbol_index", { root });
}

export async function searchSymbols(root: string, query: string, limit?: number): Promise<Symbol[]> {
  if (!isDesktopShell()) {
    throw toError({ code: "desktop_only", message: "Symbol search is only available in the desktop app." });
  }
  return call<Symbol[]>("search_symbols", { root, query, limit });
}

export async function clearSymbolIndex(): Promise<boolean> {
  if (!isDesktopShell()) return false;
  try {
    return await call<boolean>("clear_symbol_index");
  } catch {
    return false;
  }
}

export function languageForPath(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  const map: Record<string, string> = {
    rs: "rust",
    ts: "typescript",
    tsx: "tsx",
    js: "javascript",
    jsx: "jsx",
    py: "python",
  };
  return map[ext] ?? "unsupported";
}

export function isSupportedLanguage(path: string): boolean {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return ["rs", "ts", "tsx", "js", "jsx", "py"].includes(ext);
}
