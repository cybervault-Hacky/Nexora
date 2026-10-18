import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell } from "./ipc";
import type { Symbol } from "./symbolIndex";

export interface DependencyEdge {
  from: string;
  to: string;
  raw: string;
  line: number;
  verified: boolean;
  kind: string;
  note: string | null;
}

export interface FileNode {
  path: string;
  imports: DependencyEdge[];
  importedBy: string[];
}

export interface AnalysisGraph {
  version: number;
  root: string;
  nodes: FileNode[];
  edges: DependencyEdge[];
  truncated: boolean;
  unsupported: number;
  cycles: string[][];
  generatedAt: number;
}

export interface Reference {
  path: string;
  line: number;
  symbol: string;
  snippet: string;
}

export interface FileRelationships {
  path: string;
  imports: DependencyEdge[];
  importedBy: string[];
  definedSymbols: Symbol[];
  referencedBy: Reference[];
  unresolved: DependencyEdge[];
  notes: string[];
}

export interface ImpactNode {
  path: string;
  reason: string;
  evidence: string;
  line: number | null;
  confidence: string; // verified | inferred
}

export interface ImpactReport {
  target: string;
  direct: ImpactNode[];
  transitive: ImpactNode[];
  risks: string[];
  truncated: boolean;
  notes: string[];
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

export async function getDependencyGraph(root: string): Promise<AnalysisGraph> {
  if (!isDesktopShell()) {
    throw toError({ code: "desktop_only", message: "Analysis is only available in the desktop app." });
  }
  return call<AnalysisGraph>("get_dependency_graph", { root });
}

export async function refreshDependencyGraph(root: string): Promise<AnalysisGraph> {
  if (!isDesktopShell()) {
    throw toError({ code: "desktop_only", message: "Analysis is only available in the desktop app." });
  }
  return call<AnalysisGraph>("refresh_dependency_graph", { root });
}

export async function getFileRelationships(root: string, path: string): Promise<FileRelationships> {
  if (!isDesktopShell()) {
    throw toError({ code: "desktop_only", message: "Analysis is only available in the desktop app." });
  }
  return call<FileRelationships>("get_file_relationships", { root, path });
}

export async function getChangeImpact(root: string, path: string): Promise<ImpactReport> {
  if (!isDesktopShell()) {
    throw toError({ code: "desktop_only", message: "Analysis is only available in the desktop app." });
  }
  return call<ImpactReport>("get_change_impact", { root, path });
}

export async function clearAnalysisGraph(): Promise<boolean> {
  if (!isDesktopShell()) return false;
  try {
    return await call<boolean>("clear_analysis_graph");
  } catch {
    return false;
  }
}

export function confidenceLabel(confidence: string): string {
  if (confidence === "verified") return "Verified";
  return "Inferred";
}

export function edgeLabel(edge: DependencyEdge): string {
  return `${edge.from} → ${edge.to} (${edge.verified ? "verified" : "unresolved"})`;
}
