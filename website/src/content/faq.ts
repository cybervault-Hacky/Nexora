/**
 * FAQ content for `/faq`.
 *
 * Every answer is limited to what the project's source and documentation
 * actually support, and is written to match the Download, Privacy, and Security
 * pages rather than to sound better than they do. Where an answer would require
 * evidence that does not exist — a release date, a minimum specification, a
 * support address — it says the information is unavailable instead.
 *
 * Sources consulted:
 * - `apps/desktop/src-tauri/src/inference/registry.rs` — the two model profiles,
 *   their recorded byte sizes, and `locally_verified: false` on both.
 * - `apps/desktop/src-tauri/src/inference/process.rs` — loopback binding, the
 *   per-launch key, and the pinned engine build reported to the user.
 * - `apps/desktop/src-tauri/src/inference/acquisition.rs` — user-initiated
 *   downloads, size + SHA-256 gate, `.part` + no-replace publish.
 * - `apps/desktop/src-tauri/src/history/storage.rs` — `conversations.json`,
 *   caps, atomic write, no encryption.
 * - `apps/desktop/src-tauri/src/auth/{client,storage}.rs` — account network
 *   calls, credential storage, HTTPS and redirect rules.
 * - `apps/desktop/src-tauri/tauri.conf.json` — Windows desktop packaging target,
 *   `nsis`, no artefacts produced.
 * - `docs/PHASE_19`–`PHASE_21`, `docs/PUBLIC_BETA_PREPARATION_WINDOWS.md` —
 *   no installer, no approved version, NO-GO release decision.
 * - `docs/SUPPORT_AND_DIAGNOSTICS.md`, `SECURITY.md` — no verified support or
 *   security-reporting channel exists.
 * - `website/src/content/trust.ts` — the same facts as stated on `/privacy` and
 *   `/security`, which these answers must not contradict.
 */

import type { RoutePath } from "../config/routes";

export interface FaqLink {
  /** Visible link text. Must describe the destination on its own. */
  label: string;
  to: RoutePath;
}

export interface FaqItem {
  /** Anchor id, so a single answer can be linked directly. */
  id: string;
  question: string;
  /** One to three sentences. Plain text; links are declared separately. */
  answer: string;
  /** Optional "read more" destinations rendered after the answer. */
  links?: readonly FaqLink[];
}

export interface FaqCategory {
  /** Anchor id for the category heading. */
  id: string;
  title: string;
  intro?: string;
  items: readonly FaqItem[];
}

export const FAQ_CATEGORIES: readonly FaqCategory[] = [
  {
    id: "product-and-compatibility",
    title: "Product and compatibility",
    intro:
      "What the product is, and how much of it exists outside the source code.",
    items: [
      {
        id: "what-is-nexora",
        question: "What is Nexora?",
        answer:
          "Nexora is a Windows desktop application for AI-assisted coding, designed around models that run on your own machine instead of in a data centre. The application exists in source code; no Windows build has been produced, so there is nothing to install today.",
        links: [{ label: "See what exists in source", to: "/features" }],
      },
      {
        id: "desktop-or-website",
        question: "Is Nexora a Windows desktop application or a website?",
        answer:
          "Both exist in the same repository, and they are separate projects. The desktop application is the product; this website documents it. The site is static, has no accounts, and has no connection to the application as you use it.",
      },
      {
        id: "internet-connection",
        question: "Does Nexora require an internet connection?",
        answer:
          "Generating a reply is designed not to need one: inference runs against a local process. A connection is still needed to download a model, to sign in, and for the installer's first-run WebView2 setup. Treat \"works offline\" as a design intention rather than a verified property — it has not been exercised on Windows, and it does not apply to every feature.",
      },
      {
        id: "what-is-local-ai",
        question: "What does \u201clocal AI\u201d mean in Nexora?",
        answer:
          "A model file on your disk, loaded by an inference engine running on this machine and bound to the loopback address 127.0.0.1. Your prompt, the context you include, and the reply travel between the application and that local process; producing an answer does not involve a remote service.",
      },
      {
        id: "which-models",
        question: "Which models are planned or represented in the source?",
        answer:
          "Two profiles are registered: Nexora Mini (a 7B Qwen2.5-Coder model, Q4_K_M GGUF, 4.36 GiB) and Nexora Sonnet (the 14B version, same format, 8.37 GiB), both intended to run on a CPU. Both are flagged in the registry as not locally verified, neither is bundled, and the inference engine itself is supplied by you.",
        links: [{ label: "How the setup is meant to work", to: "/how-it-works" }],
      },
      {
        id: "public-install",
        question: "Can the current application be installed and used by the public?",
        answer:
          "No. There is no installer, no approved release version, no published checksum, and no signing identity. The release decision recorded in this repository is no-go.",
        links: [{ label: "See the release status", to: "/download" }],
      },
    ],
  },
  {
    id: "models-and-hardware",
    title: "Models and hardware",
    intro:
      "Why model files are large, why hardware matters, and what a completed download does and does not prove.",
    items: [
      {
        id: "why-large-downloads",
        question: "Why do model downloads require substantial storage?",
        answer:
          "The two registered profiles are 4.36 GiB and 8.37 GiB as files, before any working room for the model while it runs. Those are the sizes recorded in the application, not estimates, and they are the reason the disk-space check happens before a download starts.",
      },
      {
        id: "why-hardware-varies",
        question: "Why can hardware requirements vary?",
        answer:
          "What a machine can do depends on the model size, the context length you configure, and whether the engine runs on the CPU or a GPU. The application reports the hardware it detects, but no minimum specification has been measured or published.",
      },
      {
        id: "download-proves-nothing",
        question: "Does downloading a model prove that inference works?",
        answer:
          "No. A finished download with a matching digest proves the file arrived intact and matches the digest the application recorded. It says nothing about whether the engine can load that file or produce a usable answer.",
      },
      {
        id: "minimum-hardware",
        question: "Are minimum hardware requirements officially verified?",
        answer:
          "No. No minimum RAM, CPU, GPU, or free-disk figure has been measured on real Windows hardware, so none is published. The download page will state requirements only once they are measured rather than guessed.",
        links: [{ label: "Before you download", to: "/download" }],
      },
    ],
  },
  {
    id: "privacy-and-security",
    title: "Privacy and security",
    intro:
      "Answers drawn from the source review on the privacy and security pages.",
    items: [
      {
        id: "where-history-stored",
        question: "Where is conversation history stored, according to source evidence?",
        answer:
          "In a file called conversations.json inside the application's per-user data folder, capped at 500 conversations and 10 MiB per save, and written atomically so an interrupted save cannot leave a half-written record.",
        links: [{ label: "Data and conversations", to: "/privacy" }],
      },
      {
        id: "history-encrypted",
        question: "Is local conversation history encrypted?",
        answer:
          "No. The application adds no encryption to that file. Its protection is whatever your Windows account and disk protection provide, which is also why anything you paste into a prompt should be treated as plain text on disk.",
      },
      {
        id: "network-activity",
        question: "What network activity can occur?",
        answer:
          "Five destinations: the local engine on loopback, huggingface.co for model downloads, an account service if one was configured in the build and you are signed in, accounts.google.com if you choose Google sign-in, and this website. No analytics, telemetry, or crash-reporting client is present in either the application or the site.",
        links: [{ label: "Network connections and external services", to: "/privacy" }],
      },
      {
        id: "independent-audit",
        question: "Is Nexora independently security-audited?",
        answer:
          "No. There has been no penetration test, external audit, fuzzing campaign, or certification of the application, its account service, or this website. Nothing on the security page asserts otherwise.",
        links: [{ label: "What has not been verified", to: "/security" }],
      },
      {
        id: "checksums-and-warnings",
        question:
          "How should users interpret model checksums and Windows security warnings?",
        answer:
          "A matching SHA-256 means the file matches the digest recorded in the application's registry; it does not mean the model is trustworthy, and that recorded value has not been checked against an independently obtained copy. If Windows blocks a download or shows a SmartScreen warning, treat it as information rather than an obstacle — there is no release today that would justify disabling a security feature to install it.",
        links: [{ label: "Model integrity and its limits", to: "/security" }],
      },
    ],
  },
  {
    id: "downloads-and-release",
    title: "Downloads and release status",
    intro:
      "What exists, what does not, and what has to happen before a download link appears.",
    items: [
      {
        id: "installer-available",
        question: "Is the Windows installer available?",
        answer:
          "No. No Windows build or installer has been produced, and there is no release on any channel. A packaging target is configured in the source; a configured target is not a published file.",
        links: [{ label: "Release status", to: "/download" }],
      },
      {
        id: "verify-future-download",
        question: "How can users verify a future download?",
        answer:
          "Compare the SHA-256 published next to the file against one computed from the file you received, and check whether a signing status is stated. Both values will be published only after being computed or verified on the artifact itself, not written in advance.",
      },
      {
        id: "why-no-button",
        question: "Why is there no download button yet?",
        answer:
          "Because a button would imply a file exists, and a disabled one would imply the same. The download page states the status in words and will show real metadata — version, architecture, file name, size, checksum, date — only when all of it is true.",
        links: [{ label: "Windows download", to: "/download" }],
      },
      {
        id: "before-release",
        question: "What must be completed before release?",
        answer:
          "A real Windows build; a verified install, launch, upgrade, and uninstall on Windows 10 or 11; a model actually loaded and used to generate a response; a checksum computed from the published file; an approved version and signing identity; and release notes plus a support route.",
        links: [{ label: "What has to be ready first", to: "/download" }],
      },
    ],
  },
  {
    id: "support",
    title: "Support",
    intro:
      "Where to read, and why there is no contact address to give you yet.",
    items: [
      {
        id: "product-information",
        question: "Where can users find product information?",
        answer:
          "The features page describes what exists in source and how far each claim goes. The how it works page sets out the intended setup sequence from engine to model to first conversation.",
        links: [
          { label: "Features", to: "/features" },
          { label: "How it works", to: "/how-it-works" },
        ],
      },
      {
        id: "report-problem",
        question:
          "How can users report a problem if no verified support channel is published?",
        answer:
          "There is no verified support address, ticket portal, or response commitment, and this project will not invent one. If you are testing a build, note the details listed on the support page and keep prompts, source files, tokens, and model files out of any public post. A route will be published on the support page once one exists and is actually monitored.",
        links: [{ label: "What to record, and what not to share", to: "/support" }],
      },
      {
        id: "privacy-security-info",
        question: "Where can they find the privacy and security information?",
        answer:
          "The privacy page covers what stays on the device and what leaves it. The security page covers the controls that exist, the boundaries around them, and what has not been verified.",
        links: [
          { label: "Privacy", to: "/privacy" },
          { label: "Security", to: "/security" },
        ],
      },
    ],
  },
];
