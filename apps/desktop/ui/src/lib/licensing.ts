import { invoke } from "@tauri-apps/api/core";
import { isDesktopShell } from "./ipc";
import { recordIpcCall } from "./telemetry";

export type LicenceTier = "community" | "pro" | "team" | "enterprise";

export interface LicenceStatus {
  tier: LicenceTier;
  planName: string;
  active: boolean;
  licenceKey: string | null;
  licensee: string | null;
  expiresAt: number | null;
  features: string[];
}

export interface ActivateLicencePayload {
  licenceKey: string;
}

export interface LicenceError {
  code: string;
  message: string;
}

export interface PlanFeature {
  name: string;
  included: boolean;
}

export interface PlanDefinition {
  id: LicenceTier;
  name: string;
  tagline: string;
  price: string;
  cadence: string;
  badge?: string;
  features: PlanFeature[];
}

export const PLAN_TIERS: PlanDefinition[] = [
  {
    id: "community",
    name: "Community",
    tagline: "Free & Open-Source AI Coding Studio",
    price: "$0",
    cadence: "forever free",
    badge: "Active by Default",
    features: [
      { name: "Unlimited local GGUF inference", included: true },
      { name: "Complete offline AI chat & generation", included: true },
      { name: "Full workspace & codebase indexing", included: true },
      { name: "Single-file & multi-file patch generation", included: true },
      { name: "Zero telemetry / zero account wall", included: true },
      { name: "Commercial distribution license", included: false },
      { name: "Multi-seat team workspace sync", included: false },
      { name: "Priority enterprise model configs", included: false },
    ],
  },
  {
    id: "pro",
    name: "Developer Pro",
    tagline: "For Solo Professionals & Freelancers",
    price: "$19",
    cadence: "per month / perpetual key",
    badge: "Most Popular",
    features: [
      { name: "Unlimited local GGUF inference", included: true },
      { name: "Complete offline AI chat & generation", included: true },
      { name: "Full workspace & codebase indexing", included: true },
      { name: "Single-file & multi-file patch generation", included: true },
      { name: "Zero telemetry / zero account wall", included: true },
      { name: "Commercial distribution license", included: true },
      { name: "Deep dependency & cycle impact audits", included: true },
      { name: "Priority enterprise model configs", included: false },
    ],
  },
  {
    id: "team",
    name: "Team & Enterprise",
    tagline: "For Engineering Teams & Organizations",
    price: "$49",
    cadence: "per seat / month",
    badge: "Enterprise",
    features: [
      { name: "Unlimited local GGUF inference", included: true },
      { name: "Complete offline AI chat & generation", included: true },
      { name: "Full workspace & codebase indexing", included: true },
      { name: "Single-file & multi-file patch generation", included: true },
      { name: "Zero telemetry / zero account wall", included: true },
      { name: "Commercial distribution license", included: true },
      { name: "Multi-seat team workspace sync", included: true },
      { name: "Founder Control Centre compliance audit", included: true },
    ],
  },
];

const DEFAULT_COMMUNITY_STATUS: LicenceStatus = {
  tier: "community",
  planName: "Community Edition",
  active: true,
  licenceKey: null,
  licensee: null,
  expiresAt: null,
  features: [
    "unlimited_local_inference",
    "gguf_model_execution",
    "offline_ai_chat",
    "workspace_code_intelligence",
    "single_file_and_batch_patches",
    "ast_symbol_indexing",
  ],
};

export async function licenceGetStatus(): Promise<LicenceStatus> {
  if (!isDesktopShell()) {
    return DEFAULT_COMMUNITY_STATUS;
  }

  const start = performance.now();
  try {
    const status = await invoke<LicenceStatus>("licence_get_status");
    recordIpcCall("licence_get_status", performance.now() - start, "ok");
    return status;
  } catch (err: unknown) {
    const msg = typeof err === "object" && err !== null && "message" in err
      ? String((err as { message: unknown }).message)
      : "Failed to read licence status.";
    recordIpcCall("licence_get_status", performance.now() - start, "error", msg);
    return DEFAULT_COMMUNITY_STATUS;
  }
}

export async function licenceActivate(
  payload: ActivateLicencePayload,
): Promise<LicenceStatus> {
  if (!isDesktopShell()) {
    throw {
      code: "desktop_required",
      message: "Licence activation requires the Nexora desktop runtime.",
    };
  }

  const start = performance.now();
  try {
    const status = await invoke<LicenceStatus>("licence_activate", {
      request: payload,
    });
    recordIpcCall("licence_activate", performance.now() - start, "ok");
    return status;
  } catch (err: unknown) {
    const msg = typeof err === "object" && err !== null && "message" in err
      ? String((err as { message: unknown }).message)
      : "Failed to activate licence.";
    recordIpcCall("licence_activate", performance.now() - start, "error", msg);
    throw err;
  }
}

export async function licenceDeactivate(): Promise<LicenceStatus> {
  if (!isDesktopShell()) {
    return DEFAULT_COMMUNITY_STATUS;
  }

  const start = performance.now();
  try {
    const status = await invoke<LicenceStatus>("licence_deactivate");
    recordIpcCall("licence_deactivate", performance.now() - start, "ok");
    return status;
  } catch (err: unknown) {
    const msg = typeof err === "object" && err !== null && "message" in err
      ? String((err as { message: unknown }).message)
      : "Failed to deactivate licence.";
    recordIpcCall("licence_deactivate", performance.now() - start, "error", msg);
    throw err;
  }
}
