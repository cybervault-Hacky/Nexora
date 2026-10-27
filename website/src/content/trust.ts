/**
 * Trust content for the Privacy (`/privacy`) and Security (`/security`) pages.
 *
 * Every statement in this file was checked against the desktop application's
 * source before it was written, and the status label on each row records how
 * far that evidence goes:
 *
 * - `verified`   — supported by specific inspected code or documentation.
 * - `unverified` — implemented, but the behaviour depends on something that has
 *                  not been exercised (a Windows build, a real model, a deployed
 *                  service).
 * - `planned`    — designed or configured, not demonstrated.
 *
 * Sources consulted:
 * - `apps/desktop/src-tauri/src/inference/process.rs` — loopback bind address,
 *   ephemeral port, per-launch engine key, pinned runtime build.
 * - `apps/desktop/src-tauri/src/inference/acquisition.rs` — user-initiated
 *   downloads, pinned revisions, byte count + SHA-256 gate before publish.
 * - `apps/desktop/src-tauri/src/media_models/download.rs` — digest comparison
 *   against registry metadata.
 * - `apps/desktop/src-tauri/src/history/storage.rs` — `conversations.json`,
 *   size caps, atomic write, no encryption.
 * - `apps/desktop/src-tauri/src/usage/{mod,store}.rs` — `daily-usage.txt`
 *   contents and what that format explicitly excludes.
 * - `apps/desktop/src-tauri/src/workspace/scanner.rs` — secret-bearing file
 *   exclusions and `.gitignore` handling.
 * - `apps/desktop/src-tauri/src/auth/{client,storage,pkce}.rs` — HTTPS rules,
 *   disabled redirects, PKCE, Credential Manager storage, and the exact fields
 *   sent by a usage reservation.
 * - `apps/desktop/src-tauri/src/inference/commands.rs` — account usage is
 *   best-effort and cannot block local generation.
 * - `apps/desktop/src-tauri/{tauri.conf.json,capabilities/main.json}` — content
 *   security policy and the granted permissions.
 * - `docs/PRODUCTION_DEPLOYMENT_AND_SECURITY.md`,
 *   `docs/PHASE_17_CONFIGURATION_SECURITY.md`,
 *   `docs/LEGAL_PRIVACY_TERMS.md` — configuration, storage, and legal-status
 *   documentation, all of which label themselves pre-release and unverified.
 *
 * Do not promote a row to `verified` without executed evidence, and do not add
 * a row describing behaviour that is not in the source.
 */

/** How far the evidence behind a statement actually goes. */
export type TrustStatus = "verified" | "unverified" | "planned";

/** Visible label text. The words, not a colour, carry the meaning. */
export const TRUST_LABELS: Record<TrustStatus, string> = {
  verified: "Verified in source",
  unverified: "Not yet verified",
  planned: "Planned",
};

/** One labelled statement. */
export interface TrustFact {
  /** Short subject, shown as the list term. */
  term: string;
  /** One or two sentences in plain language. */
  detail: string;
  /** Evidence level. Omitted when the row is a plain fact rather than a claim. */
  status?: TrustStatus;
}

/** Explains the three labels once per page. */
export const TRUST_LEGEND: ReadonlyArray<{
  status: TrustStatus;
  meaning: string;
}> = [
  {
    status: "verified",
    meaning:
      "The behaviour is visible in the source code that ships with this project. It has not been observed running, and no independent party has reviewed it.",
  },
  {
    status: "unverified",
    meaning:
      "The code is there, but the claim depends on something that has not happened yet — a Windows build, a real model, or a running account service.",
  },
  {
    status: "planned",
    meaning:
      "Designed or configured but not demonstrated. Do not rely on it yet.",
  },
];

/* ──────────────────────────────────────────────────────────────────────────
 * Privacy
 * ────────────────────────────────────────────────────────────────────────── */

/** First-scan summary of what can and cannot be confirmed. */
export const PRIVACY_GLANCE: readonly TrustFact[] = [
  {
    term: "Prompts and replies",
    detail:
      "Generation is streamed from a llama-server process running on your own machine and bound to 127.0.0.1. The desktop core does not send prompt or reply text to a Nexora server.",
    status: "verified",
  },
  {
    term: "Your project files",
    detail:
      "Workspace scanning, symbol search, and patch proposals read files locally. File content is included in the context sent to the local inference process — not to a remote one.",
    status: "verified",
  },
  {
    term: "Conversation history",
    detail:
      "Saved as a plain JSON file in the application's per-user data folder. The application does not encrypt it.",
    status: "verified",
  },
  {
    term: "Analytics and crash reports",
    detail:
      "No analytics, telemetry, or crash-reporting library is present in the desktop application's dependencies or in this website's source.",
    status: "verified",
  },
  {
    term: "An account",
    detail:
      "Optional. Local generation is designed to work while signed out, and an account failure is not supposed to block it. The account service is not deployed in this repository.",
    status: "verified",
  },
  {
    term: "How any of this behaves on Windows",
    detail:
      "Unobserved. No Windows build or installer has been produced, installed, or run, so every statement above is a reading of source code rather than a measured result.",
    status: "unverified",
  },
];

/** Local model workflow: acquisition versus use. */
export const LOCAL_AI_FACTS: readonly TrustFact[] = [
  {
    term: "Downloading a model",
    detail:
      "A model file is fetched from huggingface.co at a pinned revision, and only when you start the download yourself. The file is hashed while it arrives and is installed only if its byte count and SHA-256 match the values recorded in the application.",
    status: "verified",
  },
  {
    term: "Using a model",
    detail:
      "Once a model is installed, prompts and replies travel to a local process on a loopback port. Nothing about a generation is sent off the device to produce the answer.",
    status: "verified",
  },
  {
    term: "What still needs a connection",
    detail:
      "Downloading a model, signing in, checking account usage, and the installer's first-run WebView2 setup all need a network. Nexora is not an offline-only product.",
    status: "verified",
  },
  {
    term: "Working without a connection",
    detail:
      "Local generation is designed not to depend on the account service, and a network failure is coded as a warning rather than a block. Nobody has confirmed this on a Windows machine.",
    status: "unverified",
  },
  {
    term: "The inference engine itself",
    detail:
      "Nexora does not ship the local runtime. You supply the engine executable and choose a model, which means its provenance is your decision, not something the application can vouch for.",
    status: "verified",
  },
];

/** What is written to disk on this device. */
export const DATA_STORAGE_FACTS: readonly TrustFact[] = [
  {
    term: "Conversations",
    detail:
      "A single conversations.json file, capped at 500 conversations and 10 MiB per save, written atomically. It is readable text, and the application adds no encryption of its own.",
    status: "verified",
  },
  {
    term: "Usage counter",
    detail:
      "A daily-usage.txt file holding four numbers and a date. Its format is documented as never containing prompts, replies, file contents, paths, or identifiers.",
    status: "verified",
  },
  {
    term: "Account session",
    detail:
      "Held by Windows Credential Manager rather than a file. If the credential store is unavailable, the session lives only in process memory.",
    status: "verified",
  },
  {
    term: "Workspace settings",
    detail:
      "Stored in the application's per-user configuration folder: the workspace you opened and your preferences for it.",
    status: "verified",
  },
  {
    term: "Exports",
    detail:
      "Export writes to a path you pick. The ZIP export is deliberately plain — no encryption, no extra metadata fields.",
    status: "verified",
  },
  {
    term: "Model files",
    detail:
      "Stored at the location you choose. The application records that path and re-checks the digest when you verify a file.",
    status: "verified",
  },
];

/** Every destination the desktop application can talk to. */
export const NETWORK_DESTINATIONS: readonly TrustFact[] = [
  {
    term: "This device (127.0.0.1)",
    detail:
      "The local inference engine. Carries prompts, replies, and the project context you include. Bound to the loopback interface only.",
    status: "verified",
  },
  {
    term: "huggingface.co",
    detail:
      "Model downloads at pinned revisions, started by you: the coding model, and separate model files if you use the optional image and video features.",
    status: "verified",
  },
  {
    term: "An account service",
    detail:
      "Only when you are signed in and a service address was compiled into the build. A per-generation request carries a model identifier and a request identifier — no prompt text, no file contents.",
    status: "unverified",
  },
  {
    term: "accounts.google.com",
    detail:
      "Only if you choose Google sign-in and the build includes a Google client ID. Without one, the option is unavailable.",
    status: "verified",
  },
  {
    term: "This website",
    detail:
      "No analytics, no cookies, no forms, no external fonts, and no third-party requests. Every page is served from the same origin as static files.",
    status: "verified",
  },
];

/** Credential handling, and its limits. */
export const CREDENTIAL_FACTS: readonly TrustFact[] = [
  {
    term: "Account session",
    detail:
      "Stored in Windows Credential Manager under the application's identifier. Raw session tokens are not passed to the application window.",
    status: "verified",
  },
  {
    term: "Local engine key",
    detail:
      "A fresh 64-character key is generated for each launch, written to a new file that the application refuses to overwrite, and required by the engine on every request. On Windows the file inherits the per-user application data folder's access rules.",
    status: "verified",
  },
  {
    term: "Passwords",
    detail:
      "Typed into the application and sent to the account service over HTTPS. The desktop application stores no password of its own.",
    status: "verified",
  },
  {
    term: "Secrets inside your project",
    detail:
      "The workspace scanner refuses to read secret-bearing files — .env and .env.*, .pem, .key, .p12, .pfx, .kdbx, .netrc, .git-credentials, credentials, and secrets.* — and honours your .gitignore.",
    status: "verified",
  },
  {
    term: "Anything you paste into a prompt",
    detail:
      "That text is part of the conversation and is written to the history file in plain text. The exclusions above cover file scanning; they cannot protect a secret you type.",
    status: "verified",
  },
  {
    term: "What a deployed service would hold",
    detail:
      "The account service's schema describes password hashes, session-token hashes, billing references, daily counters, and audit events. No service is running, so none of it has been observed.",
    status: "unverified",
  },
];

/** Diagnostics behaviour, stated as found. */
export const DIAGNOSTICS_FACTS: readonly TrustFact[] = [
  {
    term: "Analytics",
    detail:
      "None. The desktop crate pulls no analytics, session-replay, or product-intelligence dependency, and this website loads no third-party script.",
    status: "verified",
  },
  {
    term: "Crash reporting",
    detail:
      "None configured. A failure is reported in the interface and, for internal errors, written to the console — not uploaded and not written to a log file.",
    status: "verified",
  },
  {
    term: "Local usage record",
    detail:
      "A counter and a date, kept on this device for display. It is explicitly not an account, billing, or entitlement authority, and no local daily limit is configured.",
    status: "verified",
  },
  {
    term: "Server-side usage",
    detail:
      "Only if you are signed in against a configured service. Counters and reservations may be stored server-side; the production quota policy defaults to unconfigured.",
    status: "unverified",
  },
];

/** Practical precautions, in plain language. */
export const PRIVACY_PRECAUTIONS: readonly string[] = [
  "Use Nexora signed out if you do not need account features. Local generation is designed not to require an account.",
  "Keep secrets out of prompts. Conversation history is a plain text file on this disk, and it will contain anything you typed.",
  "Choose where model files live deliberately. They are large, they are not managed for you, and they remain after uninstalling.",
  "Review your own network traffic if you want to confirm any of this. The account service address, if one is compiled in, is visible in your firewall or proxy logs.",
  "Delete conversation history from the application when you are finished with a sensitive project. Deleting it does not remove copies made by backups or file-sync tools.",
];

/** Important unknowns, stated without softening. */
export const PRIVACY_UNKNOWNS: readonly string[] = [
  "Nothing here has been observed on a real Windows installation. No build, installer, or runtime session exists.",
  "No account service is deployed, so account data handling is described by its schema, not by captured behaviour.",
  "No retention period is documented or enforced: conversations stay until you delete them, and operating-system or cloud backups are outside the application's control.",
  "No independent audit, penetration test, or privacy certification has been performed on the application, the website, or the account service.",
  "Third-party licences and the exact redistributed artifacts have not been fully reviewed, so distribution of the application is blocked.",
];

/* ──────────────────────────────────────────────────────────────────────────
 * Security
 * ────────────────────────────────────────────────────────────────────────── */

/** The three parts of the running system and what each is trusted with. */
export const SECURITY_LAYERS: readonly TrustFact[] = [
  {
    term: "The window",
    detail:
      "A webview with a strict content security policy. Its connect-src is limited to the application's own IPC channel, so page code cannot fetch an arbitrary URL even if it tried. Its capability list grants only the core defaults plus four window controls.",
    status: "verified",
  },
  {
    term: "The desktop core",
    detail:
      "A Rust process that owns downloads, process supervision, file access, and account requests. Network access happens here, behind validation, rather than in the page.",
    status: "verified",
  },
  {
    term: "The local engine",
    detail:
      "A separate llama-server process, bound to 127.0.0.1 on a freshly chosen port, started with a key that exists only for that launch.",
    status: "verified",
  },
];

/** Controls that are present in source. */
export const SECURITY_CONTROLS: readonly TrustFact[] = [
  {
    term: "Loopback-only inference",
    detail:
      "The engine is started with an explicit loopback host argument and never bound to 0.0.0.0 or a network interface, so it is not reachable from other machines by design.",
    status: "verified",
  },
  {
    term: "Per-launch engine key",
    detail:
      "Each launch writes a new key file the process refuses to replace, and probes and requests must present it. A stale key from an unclean exit is removed at startup.",
    status: "verified",
  },
  {
    term: "Model integrity gate",
    detail:
      "A download is hashed as it streams and is published only if both the byte count and the SHA-256 match. Failed or cancelled transfers leave a .part file, never something that looks installed, and publishing never overwrites an existing file.",
    status: "verified",
  },
  {
    term: "Session storage",
    detail:
      "Account sessions live in Windows Credential Manager. A memory-only fallback is returned as an error rather than presented as a durable sign-in.",
    status: "verified",
  },
  {
    term: "Transport rules",
    detail:
      "Account requests require HTTPS. Plain HTTP is permitted only for loopback in development, and only with an explicit opt-in. Redirects are refused, so a response cannot bounce a request — or its credentials — to another host.",
    status: "verified",
  },
  {
    term: "Sign-in flow",
    detail:
      "Google sign-in uses PKCE S256 with a state value and a nonce generated from the operating system's random source, and the callback listener accepts only a loopback address.",
    status: "verified",
  },
  {
    term: "Least-privilege window",
    detail:
      "The window is granted no filesystem or HTTP plugin permissions. Anything it needs comes through a named application command.",
    status: "verified",
  },
  {
    term: "Secret file exclusions",
    detail:
      "Secrets are kept out of the workspace context by name: .env files, private keys, certificate bundles, keyrings, and credential files are excluded before reading.",
    status: "verified",
  },
  {
    term: "Edits are proposed, then validated",
    detail:
      "Changes to workspace files go through a patch proposal that is validated before it is applied, rather than being written by the assistant directly.",
    status: "verified",
  },
];

/** What is genuinely not known or not done. */
export const SECURITY_LIMITS: readonly TrustFact[] = [
  {
    term: "No independent assessment",
    detail:
      "No penetration test, external audit, fuzzing campaign, or formal review has been performed. Nothing here is certified.",
    status: "unverified",
  },
  {
    term: "No Windows runtime evidence",
    detail:
      "No installer exists, so no code path has been exercised on a Windows machine. This includes the loopback boundary, the key file, and Credential Manager.",
    status: "unverified",
  },
  {
    term: "No signing or reputation",
    detail:
      "There is no signed installer, no approved release version, and no SmartScreen reputation. A future build would start with none of those.",
    status: "unverified",
  },
  {
    term: "Model files are not signed",
    detail:
      "A digest comparison proves the bytes match the value the application recorded. It says nothing about whether the upstream model is trustworthy, well-behaved, or licensed for redistribution.",
    status: "verified",
  },
  {
    term: "Loopback is not a boundary by itself",
    detail:
      "Another process running as the same user can reach the engine's port and can read the key file; the file's protection is the folder's access rules, not encryption.",
    status: "verified",
  },
  {
    term: "History is plain text",
    detail:
      "Conversations are stored unencrypted. Any protection against another user account or a stolen disk comes from the operating system, not from Nexora.",
    status: "verified",
  },
  {
    term: "Account and billing paths are unexercised",
    detail:
      "The account, plan, and payment surfaces exist in source but have not been run against a deployed service, and the payment provider is deliberately left unconfigured.",
    status: "unverified",
  },
  {
    term: "No supply-chain inventory",
    detail:
      "There is no complete software bill of materials and no finished third-party licence review. Distribution remains blocked on that work.",
    status: "unverified",
  },
];

/** Reasonable precautions for a user. */
export const SAFE_USAGE: readonly string[] = [
  "Do not disable Windows security features or click through SmartScreen warnings to install Nexora. There is no release to install, and a warning is information rather than an obstacle.",
  "When an installer is eventually published, compare its SHA-256 against the value shown on the download page before running it.",
  "Install the local runtime from a source you trust. Nexora does not supply it, so its integrity is your decision.",
  "Treat the conversation history as readable text, and keep credentials out of prompts.",
  "Report anything that looks wrong through the support page rather than publishing details that could expose other users.",
];
