import type { IconName } from "../../components/Icon";

export type IntegrationCategory = "3d" | "development" | "design";
export type ImplementationStatus = "planned" | "implemented";
export type InstallationStatus =
  | { state: "unknown" }
  | { state: "checking" }
  | {
      state: "found";
      installations: readonly {
        executablePath: string;
        version: string | null;
        method: "standard_directory";
        confidence: "candidate_only";
        validation: string;
      }[];
      incompleteLocations: readonly string[];
      rejectedCandidates: number;
    }
  | { state: "not_found"; checkedLocations: readonly string[]; rejectedCandidates: number }
  | { state: "unsupported"; reason: string }
  | { state: "error"; code: string; message: string; incompleteLocations: readonly string[] }
  | { state: "access_denied"; reason: string; incompleteLocations: readonly string[] }; 
export type ConnectionAction = "connect" | "disconnect" | "reconnect" | "configure";

/** Mirrors the Phase 1 connector lifecycle. `null` means no connector exists to observe it. */
export type ConnectionStatus =
  | { state: "not_connected" }
  | { state: "connecting" }
  | { state: "connected"; sessionId: string; since: string }
  | { state: "needs_setup"; requirements: readonly string[] }
  | { state: "unsupported"; reason: string }
  | { state: "error"; message: string; retryable: boolean }
  | { state: "disconnected"; reason: string; wasConnected: boolean };

export type TaskReadiness =
  | { state: "ready"; capabilityIds: readonly string[] }
  | { state: "blocked"; reasons: readonly string[] }
  | { state: "unknown"; reason: string }
  | { state: "unavailable"; reason: string };

export interface AppCapability {
  id: string;
  name: string;
  description: string;
  status: "planned" | "available";
}

export interface AppIntegrationDefinition {
  /** Stable Nexora catalogue key; not a connection or installation identifier. */
  id: string;
  name: string;
  description: string;
  category: IntegrationCategory;
  keywords: readonly string[];
  icon: IconName;
  capabilities: readonly AppCapability[];
  compatibility: {
    platform: string;
    versionRange: string | null;
    verified: boolean;
    note: string;
  };
  implementationStatus: ImplementationStatus;
  installationStatus: InstallationStatus;
  /** Null is intentional when no connector exists; it does not mean disconnected. */
  connectionStatus: ConnectionStatus | null;
  taskReadiness: TaskReadiness;
  /** Only actions backed by a real registered lifecycle may be listed here. */
  connectionActions: readonly ConnectionAction[];
  setupRequirements: readonly string[];
  limitations: readonly string[];
}

const NOT_IMPLEMENTED = "No connector implementation is available in this build.";
const UNVERIFIED_COMPATIBILITY =
  "Application and connector compatibility have not been verified.";

/**
 * Static, data-driven catalogue metadata only. It performs no detection,
 * connection, IPC, file access, or app control. Planned capability labels are
 * goals, not currently supported features.
 */
export const APP_INTEGRATIONS = [
  {
    id: "blender",
    name: "Blender",
    description: "A planned path for working with 3D scenes from Nexora AI Chat.",
    category: "3d",
    keywords: ["modeling", "scene", "mesh", "materials", "lighting", "animation", "image to 3d"],
    icon: "apps",
    capabilities: [
      {
        id: "scene-inspection",
        name: "Scene inspection",
        description: "Review selected scene and object information.",
        status: "planned",
      },
      {
        id: "procedural-scenes",
        name: "Procedural scene creation",
        description: "Create bounded objects, furniture, environments, materials, and lighting.",
        status: "planned",
      },
      {
        id: "scene-revisions",
        name: "Scene revisions and animation",
        description: "Revise supported scene elements and later-tested object or rig workflows.",
        status: "planned",
      },
      {
        id: "reference-and-image-to-3d",
        name: "Reference images and image-to-3D",
        description: "Requires separate image-understanding or 3D-generation capability; results would be approximate.",
        status: "planned",
      },
    ],
    compatibility: {
      platform: "Windows target",
      versionRange: null,
      verified: false,
      note: UNVERIFIED_COMPATIBILITY,
    },
    implementationStatus: "planned",
    installationStatus: { state: "unknown" },
    connectionStatus: null,
    taskReadiness: { state: "blocked", reasons: ["No validated Blender connector, handshake, or task executor is available."] },
    connectionActions: [],
    setupRequirements: [
      "A supported Blender version and reviewed connector/add-on have not been established.",
      "Windows installation detection runs only after you select Check Installation; it reports candidates, not a verified Blender identity.",
      "No connection handshake or task executor is available.",
    ],
    limitations: [
      "Nexora cannot currently inspect or control Blender.",
      "Image-to-3D, animation, and scene operations are not implemented.",
    ],
  },
  {
    id: "vscode",
    name: "Visual Studio Code",
    description: "A planned candidate for scoped development-app context workflows.",
    category: "development",
    keywords: ["code", "editor", "workspace", "source", "project", "development"],
    icon: "apps",
    capabilities: [
      {
        id: "workspace-context",
        name: "Scoped workspace context",
        description: "Explore a future user-approved context handoff to Nexora AI Chat.",
        status: "planned",
      },
      {
        id: "editor-context",
        name: "Editor context",
        description: "Explore future workflows for user-selected editor content.",
        status: "planned",
      },
    ],
    compatibility: {
      platform: "Windows target",
      versionRange: null,
      verified: false,
      note: UNVERIFIED_COMPATIBILITY,
    },
    implementationStatus: "planned",
    installationStatus: { state: "unknown" },
    connectionStatus: null,
    taskReadiness: { state: "unavailable", reason: NOT_IMPLEMENTED },
    connectionActions: [],
    setupRequirements: [
      "No connector or installation detector is available.",
      "Any future access would require explicit workspace and file permissions.",
    ],
    limitations: ["Nexora cannot currently connect to or control Visual Studio Code."],
  },
  {
    id: "figma",
    name: "Figma",
    description: "A planned candidate for user-approved design context workflows.",
    category: "design",
    keywords: ["design", "prototype", "frames", "assets", "ui", "ux"],
    icon: "apps",
    capabilities: [
      {
        id: "design-context",
        name: "Design context",
        description: "Explore a future user-approved design-reference handoff to Nexora AI Chat.",
        status: "planned",
      },
      {
        id: "design-assets",
        name: "Selected design assets",
        description: "Explore future export workflows subject to provider, API, and permission review.",
        status: "planned",
      },
    ],
    compatibility: {
      platform: "Windows target",
      versionRange: null,
      verified: false,
      note: UNVERIFIED_COMPATIBILITY,
    },
    implementationStatus: "planned",
    installationStatus: { state: "unknown" },
    connectionStatus: null,
    taskReadiness: { state: "unavailable", reason: NOT_IMPLEMENTED },
    connectionActions: [],
    setupRequirements: [
      "No connector or installation detector is available.",
      "Any future design-data access would require explicit user consent and a verified integration path.",
    ],
    limitations: ["Nexora cannot currently connect to or control Figma."],
  },
] as const satisfies readonly AppIntegrationDefinition[];

export type AppIntegration = (typeof APP_INTEGRATIONS)[number];
export type IntegrationFilterCategory = IntegrationCategory | "all";

export interface IntegrationSearchOptions {
  query?: string;
  category?: IntegrationFilterCategory;
}

/** Search names, descriptions, categories, keywords, and declared capabilities. */
export function filterAppIntegrations<T extends AppIntegrationDefinition>(
  entries: readonly T[],
  { query = "", category = "all" }: IntegrationSearchOptions = {},
): T[] {
  const normalizedQuery = query.trim().toLowerCase();
  const terms = normalizedQuery.split(/\s+/).filter(Boolean);

  return entries.filter((entry) => {
    if (category !== "all" && entry.category !== category) return false;
    if (terms.length === 0) return true;

    const searchableText = [
      entry.id,
      entry.name,
      entry.description,
      entry.category,
      ...entry.keywords,
      ...entry.capabilities.flatMap((capability) => [
        capability.id,
        capability.name,
        capability.description,
      ]),
    ]
      .join(" ")
      .toLowerCase();

    return terms.every((term) => searchableText.includes(term));
  });
}

export function findDuplicateIntegrationIds(
  entries: readonly Pick<AppIntegrationDefinition, "id">[],
): string[] {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const { id } of entries) {
    if (seen.has(id)) duplicates.add(id);
    seen.add(id);
  }
  return [...duplicates];
}

export const CONNECTION_STATUS_LABELS: Record<ConnectionStatus["state"], string> = {
  not_connected: "Not connected",
  connecting: "Connecting",
  connected: "Connected",
  needs_setup: "Needs setup",
  unsupported: "Unsupported",
  error: "Error",
  disconnected: "Disconnected",
};

export const INTEGRATION_CATEGORIES: ReadonlyArray<{
  id: IntegrationFilterCategory;
  label: string;
}> = [
  { id: "all", label: "All categories" },
  { id: "3d", label: "3D & modeling" },
  { id: "development", label: "Development" },
  { id: "design", label: "Design" },
];
