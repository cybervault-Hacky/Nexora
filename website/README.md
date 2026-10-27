# Nexora Official Website

This directory contains the official public website for **Nexora**, a Windows
desktop AI coding assistant designed around local-first, offline-capable
workflows.

The website's job, once complete, is to:

- explain what Nexora is;
- describe capabilities that have actually shipped, clearly separating them
  from planned, experimental, or unavailable features;
- present the product experience;
- make Nexora discoverable through search engines; and
- host the official Windows desktop download.

## Repository relationship

The website **shares the existing Nexora Git repository**. It is not a separate
repository, and `website/` is not a Git repository of its own.

```
Nexora/                  existing repository (single Git repository)
├── apps/desktop/        Windows desktop application (Tauri 2 + React) — untouched
├── services/nexora-auth/ auth/billing service (Rust) — untouched
├── docs/, scripts/, tools/, resources/
└── website/             official website — this directory
```

The separation is logical, not a new repository. The desktop application, its
Rust/Tauri code, Blender integration, chat and image-reference work, and
media-generation modules are **not** part of the website and must not be
changed as part of website work.

## Website vs. desktop application

| | Website (`website/`) | Desktop application (`apps/desktop/`) |
| --- | --- | --- |
| Purpose | Public marketing, documentation, download | The Nexora product itself |
| Platform | Web browser | Windows desktop |
| Stack | React + TypeScript + Vite | Tauri 2 (Rust) + React, black-only theme |
| Theme | Light, warm-white, charcoal type | Dark |
| Downloads | Will host the official Windows installer in a later phase | Ships the installer |

Do not copy desktop application code into this directory, and do not add
website dependencies to the desktop application's manifest.

## Stack

- **React** 19.3.1
- **TypeScript** 5.9.3 (strict)
- **Vite** 7.3.7 with `@vitejs/plugin-react` 5.2.0
- Plain CSS with custom-property design tokens (no CSS framework, no UI or
  animation library)

No backend, database, authentication, payments, analytics, telemetry, or
external AI API is used. The site requires no API keys or environment
variables to install, develop, or build, and makes no network requests at
runtime.

## Requirements

- **Node.js** 22.12.0 or newer (developed and verified against v22.22.3)
- **pnpm** 9.15.4 — the version this repository pins via `packageManager`

If `pnpm` is not installed globally, run the commands below through Corepack:

```bash
corepack pnpm@9.15.4 install
```

## Dependency isolation (important)

The repository root declares a pnpm workspace limited to `apps/*`. Without a
local workspace file, pnpm would walk up from `website/`, treat the root
workspace as its scope, and write to the root `pnpm-lock.yaml` and root
`node_modules/`.

`website/pnpm-workspace.yaml` therefore declares this directory as its own
single-project workspace. This keeps the website's `node_modules/`,
`pnpm-lock.yaml`, and install output entirely inside `website/`.

**Do not delete `website/pnpm-workspace.yaml`**, and do not add `website/*` to
the root workspace configuration without explicit owner approval.

All install and build commands must be run **from inside `website/`**:

```bash
cd website
```

## Installation

```bash
pnpm install
```

This creates `website/node_modules/` from `website/pnpm-lock.yaml`.

## Development server

```bash
pnpm dev
```

Serves the site at <http://localhost:5173/> (bound to `0.0.0.0` so the
container preview can reach it).

## Production build

```bash
pnpm build
```

Runs the TypeScript check first, then emits static files to
`website/dist/`. Output is build-verified locally only; nothing is deployed.

The build also emits `dist/robots.txt` and — only when a production origin is
configured — `dist/sitemap.xml`. Both are generated from the same public
configuration that sets the page-level indexing directive; see
"Phase 14 — SEO and discoverability" below.

## Type checking

```bash
pnpm typecheck
```

Runs `tsc --noEmit` against `src/` and `vite.config.ts`.

## Previewing a production build

```bash
pnpm preview
```

## Linting

No lint command is configured yet. `pnpm build` is gated on the TypeScript
check, which remains the only static analysis for this directory. A linter may
be added in a later phase if the owner approves it.

## Design system

The visual system is CSS-first: no component framework, no CSS framework, no
font package, no icon library, and no animation library.

### Stylesheets

`src/styles/global.css` is a thin composition layer that imports, in order:

| File | Responsibility |
| --- | --- |
| `src/styles/tokens.css` | All design tokens: colour, typography, spacing, layout, shape, depth, motion. Single source of truth. |
| `src/styles/base.css` | Reset and element-level defaults (headings, body, links, media, forms, focus, reduced motion). |
| `src/styles/utilities.css` | A few layout helpers: `.container`, `.section`, `.measure`, `.visually-hidden`, `.skip-link`. |

Component styles sit next to their components (`Button.css`, `Surface.css`,
`Eyebrow.css`, `src/App.css`) and are imported by the component module.

**Rule for later phases:** components consume tokens via `var(--…)` and must
not hard-code colours, spacing values, or radii.

### Typography

- System sans-serif stack only (`-apple-system`, `Segoe UI`, …). No webfont is
  downloaded, so first paint is never blocked on a font request and there is
  no font licensing to manage.
- Fluid scale via `clamp()` — `--text-2xs` through `--text-4xl`. The H1 ranges
  from 34px to 52px and the hero display from 40px to 72px, so neither
  overflows a 320px viewport.
- Headings use `--leading-tight` (1.1) with negative tracking; body copy uses
  `--leading-relaxed` (1.65) and a `62ch` measure.
- The UI refinement moved the small end of the scale up one step (13px → 14px
  supporting text, 15px → 16px UI text, 18px → 21px lead) and raised the
  heading steps. On a light page the previous sizes read small and tightly
  packed rather than quiet; the ladder is now body 16 / supporting 14 / lead
  18–21 / h2 28–38 / h1 34–52 / display 40–72.
- Weights: 400 / 500 / 600 / 700, used deliberately (600 for headings and
  labels, 500 for controls, 400 for body).

### Colour and measured contrast

Light-first, one accent (`--color-accent`), muted status colours, very subtle
borders. Contrast measured with the WCAG 2.1 relative-luminance formula:

| Pair | Ratio | Grade |
| --- | --- | --- |
| Primary text `#16181d` on white | 17.76:1 | AAA |
| Secondary text `#51555d` on white | 7.48:1 | AAA |
| Secondary text on warm white `#faf9f7` | 7.11:1 | AAA |
| Muted text `#6b6f77` on white | 5.04:1 | AA |
| Muted text on warm white | 4.79:1 | AA |
| Accent `#1a56db` on white | 6.18:1 | AA |
| Accent hover `#1445b3` on white | 8.29:1 | AAA |
| White on primary button `#16181d` | 17.76:1 | AAA |
| White on primary button hover `#2a2d33` | 13.80:1 | AAA |
| Success `#14713f` / warning `#7a5100` / danger `#ac2c22` on white | 6.06 / 6.99 / 6.69:1 | AA |

All body, secondary, muted, accent, and button pairs meet **WCAG AA for normal
text**; most meet AAA. Status colours are always paired with text so state is
never communicated by colour alone.

### Spacing, layout and shape

- 4px-based spacing ramp `--space-1` … `--space-12`; no arbitrary margins.
- Fluid section rhythm (`--section-space`), page gutters
  (`--page-padding`: 20px → 48px), and content widths
  (`--container-max` 1216px, `--container-narrow` 736px, `--reading-width` 62ch).
- Adjacent sections never stack their padding: `.section + .section` removes the
  second top padding, so an intro and the content below it sit one rhythm apart
  instead of two.
- Restrained radii (4/6/10/14/20px, pill reserved for tags), subtle
  low-opacity shadows, no glows.
- Touch-safe control heights: `md` is 44px.

### UI primitives

Exported from `src/components/ui/index.ts`:

| Primitive | Notes |
| --- | --- |
| `Button` | `variant: primary \| secondary`, `size: sm \| md \| lg`, `block`. Renders `<a>` when `href` is passed, `<button>` otherwise (defaults `type="button"`). Native `disabled` styling; no disabled state on links. |
| `Container` | `width: default \| narrow \| wide`, `as` element override, applies page gutters. |
| `Section` | `spacing: default \| tight \| flush`, `labelledBy` for `aria-labelledby`. |
| `Surface` | `elevation: flat \| raised \| floating`, `padding: none \| sm \| md \| lg`. Used sparingly. |
| `Eyebrow` | Small uppercase metadata label (`<p>`). |

`Button` is rendered in exactly one place: the `Back to home` action on the
not-found page. No other page carries a call-to-action button, because there is
no destination that would justify one — a download button, for example, would be
a fake control until an installer exists.

### Accessibility baseline

- Skip link to `#main` as the first focusable element.
- Semantic landmarks (`header`, `main`, `footer`), one `h1`, no skipped
  heading levels.
- `:focus-visible` outlines are never globally suppressed; focus ring tokens
  are 2px at 3px offset in the accent colour.
- `prefers-reduced-motion` neutralises every transition introduced so far.
- Transitions only, plus the single decorative home background (Phase 11) —
  still no scroll-triggered animation.

## Site architecture

The website is a **multi-page application**: every approved destination is its
own page with its own URL, rendered by React Router on top of a single shared
layout. Navigation uses real routes, not homepage section anchors.

```
index.html
└── src/main.tsx                 BrowserRouter
    └── src/App.tsx              route table (path -> page element)
        ├── components/layout/   SiteLayout (chrome + <main> + <Outlet>),
        │                        SiteHeader, SiteFooter
        ├── pages/               HomePage, FeaturesPage, HowItWorksPage,
        │                        DownloadPage, PrivacyPage, SecurityPage,
        │                        AboutPage, FaqPage, SupportPage, NotFoundPage
        ├── components/motion/   CustomCursor (Phase 12)
        ├── components/sections/ HeroSection, ParticleField (Phase 11),
        │                        PageIntro, PageNotice, FactList,
        │                        FaqCategory, TrustLabel, TrustLegend,
        │                        TrustSection
        ├── components/ui/       Button, Container, Section, Surface, Eyebrow
        ├── components/brand/    ApertureMark
        ├── config/routes.ts     canonical paths + primary navigation
        └── hooks/               useDocumentTitle
```

### Routes

| Path | Page | Navigation label |
| --- | --- | --- |
| `/` | `HomePage` (Phase 4 hero) | — (brand mark links here) |
| `/features` | `FeaturesPage` | Features |
| `/how-it-works` | `HowItWorksPage` | How It Works |
| `/download` | `DownloadPage` | Download |
| `/privacy` | `PrivacyPage` | Privacy |
| `/security` | `SecurityPage` | Security |
| `/about` | `AboutPage` | About |
| `/faq` | `FaqPage` | FAQ |
| `/support` | `SupportPage` | Support |
| `*` | `NotFoundPage` | — |

`src/config/routes.ts` is the single source of truth for paths: the route
table, the header, and the footer all import `ROUTES`, so a URL cannot drift
between the router and its links. Adding a page means adding one entry to
`ROUTES`, one element to the route table in `App.tsx`, and — if it belongs in
the navigation — one entry to `NAV_ITEMS`.

### Hosting requirement: SPA fallback (not yet configured)

The site is a static bundle that uses the History API. **Any host that serves
it must rewrite unknown paths to `/index.html`**, otherwise `/features` and
every other deep link will 404 on a page refresh or a direct visit:

| Host type | Required configuration |
| --- | --- |
| Vite dev server | none — `appType: "spa"` is the default and already rewrites |
| Vite `preview` | none — same default |
| Netlify | `/* /index.html 200` redirect |
| Vercel | `{ "rewrites": [{ "source": "/(.*)", "destination": "/index.html" }] }` |
| GitHub Pages | no SPA rewrite — needs a `404.html` copy of `index.html`, or hash routing |
| Nginx | `try_files $uri $uri/ /index.html;` |

Deployment is **not** configured and no host configuration files have been
added (Phase 14 deliberately added none). Three consequences to plan for:

1. unknown paths currently return **HTTP 200** with the not-found page
   rendered client-side, because the dev server rewrites them to `index.html`;
2. a genuine **404 status** requires host-side configuration — the `noindex`
   metadata on the fallback route is not a substitute for a status code;
3. the crawl files the Phase 14 build emits (`dist/robots.txt`, and
   `dist/sitemap.xml` once a domain is configured) must be served from the
   domain root, and must not be swallowed by the SPA rewrite.

The `/admin`-serving Node process (`server/index.mjs`, see below) implements
this behaviour itself — extensionless paths return the shell, unknown paths
with a file extension return `404`, and `/robots.txt` / `/sitemap.xml` are
either the real file or an honest `404`. The table above applies to hosting the
static build *without* that process. The full launch checklist, including which
routing rules a host must preserve and why, is in `DEPLOYMENT.md`.

### Navigation configuration

`NAV_ITEMS` in `src/config/routes.ts` drives the desktop link list and the
mobile menu. Home is intentionally not in the list: the brand mark links to
`/`, and repeating it would duplicate that destination.

The footer does **not** repeat the navigation. It carries the brand, one
sentence about the product, and — below a hairline rule — links to `/privacy`
and `/security` only, because a second copy of the header's eight links adds no
destination and only noise.

`NavLink` sets `aria-current="page"` on the active destination, so the current
page is announced by assistive technology and never signalled by colour alone.

### Features page (`/features`)

`src/pages/FeaturesPage.tsx` renders five capability groups from
`src/content/capabilities.ts`, each entry carrying an explicit status label.

**The status labels are the point of the page.** "A feature exists in source"
and "a feature works for you" are different claims, and the desktop application
has never been run in a verified environment. Three labels are used:

| Label | Meaning |
| --- | --- |
| `In source` | Code exists and is covered by the project's automated UI tests, which use simulated responses. Not verified end to end. |
| `Not yet verified` | Code exists, but the capability depends on something unverified — loading a real model, or running on real Windows hardware. |
| `Planned` | Designed or configured but not demonstrated. |

Current inventory: 15 `In source`, 4 `Not yet verified`, 3 `Planned`. **No
capability is presented as verified working**, and there is no
"verified/available" badge tier.

A `CapabilityLegend` component explains the labels on-page, and a closing
"what this page does not claim" section states the negatives explicitly rather
than leaving them implied.

#### Evidence behind each group

| Group | Grounding |
| --- | --- |
| Local model workflow | `src-tauri/src/inference/process.rs` (`LOOPBACK_HOST = "127.0.0.1"`, enforced `--host`), `client.rs` (streaming + cancellation), `registry.rs` (two profiles, pinned sizes/digests; both `locally_verified: false`) |
| Coding assistance | `workspace/patch.rs` (propose → validate → apply, content hash), `patch_batch`, `analysis.rs` (dependency graph, relationships, change impact), `symbol_index.rs` |
| Model selection and management | `components/ModelSelector.tsx` + `.test.tsx`, `ModelSetup.tsx` + `.test.tsx`, `inference/acquisition.rs` (`.part` staging, SHA-256 verify, rename), `readiness.rs` / `hardware.rs` |
| Privacy and control | `inference/process.rs` loopback binding, `App.offlineRegression.test.tsx` (mocked signed-out behaviour), `history/storage.rs` (`conversations.json`) |
| Windows desktop experience | `tauri.conf.json`, NSIS config — but `docs/PHASE_19_WINDOWS_PACKAGING.md` records that **no Windows binary or installer was produced** |

Cross-cutting: `docs/PHASE_18_INTEGRATION_VERIFICATION.md` — typecheck, 88
Vitest files / 870 tests and the production build passed; native Rust, Tauri
launch, real IPC, local inference, OAuth, payments and PostgreSQL were not run.

#### Content rules for this page

- Never add a capability without identifying the file or test that supports it.
- Never raise a status without new *executed* evidence. A passing mocked test,
  a plausible source file, or a roadmap entry is not end-to-end verification.
- Keep the "what this page does not claim" section in sync when claims change.
- Do not mention model names beyond the two actual profiles, and do not imply
  an installer, benchmark, certification, or support channel exists.

### How It Works page (`/how-it-works`)

`src/pages/HowItWorksPage.tsx` renders a five-step sequence from
`src/content/workflow.ts`, each step labelled with the same
`CapabilityBadge` used on `/features`, plus an explicit evidence line.

Structure: `h1` ("Understand the workflow, before you begin.") → a "What is not
yet verified" callout → the conceptual workflow diagram → `h2` "The sequence"
with five `h3` steps as a semantic `<ol>` → `h2` "What stays on your machine".

#### Facts on this page are cited, not estimated

| Statement | Source |
| --- | --- |
| Mini is 4,683,073,536 bytes (4.36 GiB); Sonnet is 8,988,110,272 bytes (8.37 GiB) | `inference/registry.rs` (`size_bytes`) |
| Both are Q4_K_M GGUF of Apache-2.0 Qwen2.5-Coder, intended to run on CPU | `registry.rs` (`quantization`, `licence`, `compatibility_notes`) |
| The engine executable is user-supplied; Nexora stores its path | `inference/settings.rs` (`runtime_path: Option<String>`) |
| Downloads are user-initiated, staged in `.part`, SHA-256 verified, then renamed | `inference/acquisition.rs` |
| Readiness figures are arithmetic estimates; unknown memory/disk is reported as `unknown`, not "fine" | `inference/readiness.rs` module docs |
| Runtime binds `127.0.0.1`; session credential passed via `--api-key-file`, not argv | `inference/process.rs` (`build_launch_args`) |
| No Windows installer exists | `docs/PHASE_19_WINDOWS_PACKAGING.md`; no artifact in the repo |

No download times, minimum hardware specification, or compatibility
matrix is published, because none is verified.

#### Diagram

`WorkflowDiagram.tsx` is a three-stage conceptual flow — Windows application →
local model → local inference (127.0.0.1) — with a visible caption: *"Conceptual
workflow — not a screenshot or evidence of a verified Windows runtime."* The
graphic is `aria-hidden` (the ordered list carries the same information) and
uses copy only: no canvas, no WebGL, no 3D, no animation.

### Windows Download page (`/download`)

`src/pages/DownloadPage.tsx` renders a deliberate **unavailable** state. After
the intro there are four sections — release status, release prerequisites, a
pre-download checklist, and verification — plus links to `/features` and
`/how-it-works`.

#### No installer exists (verified 2026-10-27)

| Check | Result |
| --- | --- |
| `find` for `.exe`, `.msi`, `.msix`, `SHA256SUMS*`, release manifests | **none** |
| `.exe` / `.msi` anywhere in Git history | **none** |
| `gh release list` | **no releases** |
| `docs/PHASE_19_WINDOWS_PACKAGING.md` | "no Windows installer was produced" |
| `docs/PHASE_20_PUBLISHING_PREPARATION.md` | no tags, releases, changelog, or notes found |
| `docs/PHASE_21_FINAL_APPROVAL_AND_RELEASE.md` | **NO-GO — RELEASE BLOCKED**; `0.1.0` is *not* an approved release version |

A configured NSIS target in `tauri.conf.json` is build configuration, not an
artifact. The page therefore shows **no download control of any kind** — not
even a disabled one, because a greyed-out button still reads as a control that
ought to work.

#### Release metadata is typed but empty

`src/content/release.ts` exports `CURRENT_RELEASE: ReleaseMetadata | null`,
which is **`null`**. `ReleaseInfo` renders **nothing at all** while it is null —
no empty field grid and no dash placeholders, because missing cells read as data
that ought to be there.

To publish a release later, populate that object **only** from a real published
installer, with values measured from that exact file:

```ts
export const CURRENT_RELEASE: ReleaseMetadata = {
  version: "…",        // approved version, not the manifest default
  architecture: "x64", // the architecture actually built
  fileName: "…",       // exact published filename
  sizeBytes: 0,        // measured from the artifact
  sha256: "…",         // computed from that artifact, then re-checked
  releasedAt: "…",     // ISO date
};
```

Do not derive any field from `package.json`, `tauri.conf.json`, a CI job, or a
previous report. When `CURRENT_RELEASE` becomes non-null, `ReleaseStatus`
switches to its available state automatically.

#### Content rules

- Never add a download link, `download` attribute, or hosted file reference
  until the artifact exists and is published.
- Never publish a checksum that was not computed from the published file.
- Never claim a digital signature, malware scan, certification, or audit
  without artifact-level evidence.
- Never advise disabling Windows security features or bypassing SmartScreen —
  the page currently warns *against* doing so.

### Removed: the lower home section (UI refinement)

An earlier build carried a second home section — `ProductShowcase` plus
`LocalWorkspaceDiagram` — beneath the hero: a repeated eyebrow and `h2`, a
restatement of the same description, a three-row definition list, and a 2D
workflow figure. It duplicated the hero's points and doubled the length of the
page, so it was removed rather than hidden: the components and their
stylesheets are deleted and `HomePage` renders `HeroSection` alone.

Nothing else used them. The workflow figure that remains on `/how-it-works`
(`WorkflowDiagram`) is a separate component and is untouched, so the intended
sequence is still illustrated where it belongs.

Also removed: the `HeroAperture` composition that sat to the right of the hero
copy. The home page now has no illustration at all.


### Trust pages (`/privacy` and `/security`)

Phase 9 replaces the Privacy placeholder with two pages that describe the
desktop application's actual behaviour, drawn from its source and labelled with
how far each statement goes.

- `/privacy` — *Where your code and conversations go.* Nine sections: at a
  glance, local AI and model downloads, data and conversations, network
  connections, credentials, analytics and diagnostics, your controls, current
  limitations, and questions.
- `/security` — *What Nexora does to protect your machine.* Nine sections: the
  approach, how the application is put together, controls present in source, the
  network boundary and what it does not cover, model integrity, Windows
  installation and signing, what has not been verified, reporting a problem, and
  safe usage.

`/security` is a **separate route**, not a section of `/privacy`. Every approved
destination in this site is its own page with a stable URL, both pages are long
enough to stand alone, and the security material is materially different from
the privacy material. The route is registered in `src/config/routes.ts`
(`ROUTES.security`), added to `NAV_ITEMS`, and rendered by `App.tsx`; the header,
mobile panel, footer, and active-link behaviour follow automatically because all
of them read that one list.

#### Evidence labels

`src/content/trust.ts` is the single source for every statement on both pages,
and each row carries a `TrustStatus`:

| Status | Label | Meaning |
| --- | --- | --- |
| `verified` | **Verified in source** | Supported by inspected code or configuration. Not a runtime test, audit, or certification. |
| `unverified` | **Not yet verified** | Present in source but depends on something that has not been exercised (a Windows build, a real model, a deployed service). |
| `planned` | **Planned** | Designed or configured only. |

`TrustLabel` renders the state as **words with no colour coding, no pill, and no
icon** — deliberately unlike `CapabilityBadge`, so nothing on these pages can be
mistaken for an approval or certification mark. `TrustLegend` explains the three
labels once, near the top of each page.

#### Evidence behind the claims

| Claim | Source |
| --- | --- |
| Inference is bound to `127.0.0.1` on an ephemeral port and never `0.0.0.0` | `inference/process.rs` (`LOOPBACK_HOST`, `build_launch_args`) |
| Each launch generates a fresh key, written create-new, `0600` on Unix, inherited ACL on Windows | `inference/process.rs` (`write_key_file`) |
| Model downloads are user-initiated, hashed while streaming, and published only on a size + SHA-256 match | `inference/acquisition.rs` |
| Media-model downloads compare a registry digest, and stay "unvalidated" without one | `media_models/download.rs` |
| History is `conversations.json`, plain text, 500 conversations / 10 MiB, atomic write | `history/storage.rs` |
| `daily-usage.txt` holds four numbers and a date — never prompts, replies, paths, or identifiers | `usage/{mod,store}.rs` |
| A session lives in Windows Credential Manager, with a non-durable memory fallback | `auth/storage.rs` |
| A usage reservation sends only a model id and a request id | `auth/client.rs` (`reserve_usage`) |
| Account failure cannot block local generation | `inference/commands.rs` |
| HTTPS required, redirects refused, PKCE S256 + state + nonce | `auth/{client,pkce}.rs` |
| `connect-src ipc: http://ipc.localhost`, no fs/http plugin permissions | `tauri.conf.json`, `capabilities/main.json` |
| Secret-bearing files excluded from the workspace scan | `workspace/scanner.rs` |
| No analytics, telemetry, crash reporter, or log file | dependency list + call-site search; no such client found |

#### What these pages deliberately do not contain

No legal entity, registered address, data-protection officer, contact address,
effective date, or jurisdiction claim — none exists. No certification, audit
result, signing claim, or security guarantee. No invented reporting channel,
response time, or bounty programme. `/privacy` opens with a notice stating it is
an informational overview that must be replaced by a reviewed policy before the
application is distributed.

Both pages are static text: no analytics, no cookies, no forms, no external
requests, and no images.

### About, FAQ and Support (`/about`, `/faq`, `/support`)

Phase 10 replaces the last three placeholders. All three pages reuse
`PageIntro`, `TrustSection`, `FactList`, and `PageNotice` from Phase 9, so no new
layout system was introduced.

- `/about` — *About the Nexora project.* What the product is, why it is built
  around local models (including the cost of that trade), how the application,
  model, and engine relate, four design principles, the current development
  status by evidence tier, an explicit "what Nexora does not claim" list, and
  links onward. Content lives in `src/content/about.ts`.
- `/faq` — *Frequently asked questions.* 22 questions in five categories
  (product and compatibility, models and hardware, privacy and security,
  downloads and release status, support). Content lives in `src/content/faq.ts`.
- `/support` — *Getting help with Nexora.* Support status, six failure modes the
  source anticipates, a report checklist and a "what not to share" list drawn
  from `docs/SUPPORT_AND_DIAGNOSTICS.md`, the distinction between what a user can
  investigate now and what needs a verified release, cross-links, and what will
  change when a real channel exists.

#### FAQ disclosures

Questions are native `<details>`/`<summary>` (`src/components/sections/
FaqCategory.tsx`). The browser supplies keyboard operation, focus order, and the
expanded state, so **no script and no dependency** are involved, and the answers
are in the HTML even when collapsed — browser search and screen readers reach
them. The plus/minus marker is drawn from two CSS borders and is `aria-hidden`,
because the native state is already announced.

#### No support channel is claimed

`SECURITY.md` and `docs/SUPPORT_AND_DIAGNOSTICS.md` both label the support and
security-reporting channel as a **release-blocking placeholder**, and
`docs/PUBLIC_BETA_PREPARATION_WINDOWS.md` records **NO-GO**. The repository has
GitHub issues enabled, but no document establishes it as a monitored support
route, and `SECURITY.md` explicitly says not to send vulnerability details to a
public issue. The Support page therefore publishes **no** email address, portal,
phone number, office address, community link, response time, or bounty — and says
so. Nothing on it makes a network request or posts a form.

#### Consistency with earlier phases

The three pages restate, rather than re-derive, facts already published: no
Windows build or installer, `127.0.0.1` inference, user-supplied engine, model
sizes 4.36 GiB / 8.37 GiB with both profiles not locally verified,
`conversations.json` stored unencrypted, five network destinations, no
independent audit, and no approved release version or checksum. The Phase 10
render suite asserts these strings appear, so a future edit that changes one page
without the others fails the suite.

### Page content status

| Route | State |
| --- | --- |
| `/` | Hero only (Phase 4; lower showcase removed in the UI refinement) |
| `/features` | **Real content** (Phase 6): five evidence-backed capability groups with status labels |
| `/how-it-works` | **Real content** (Phase 7): five-step workflow with evidence labels + conceptual diagram |
| `/download` | **Real content** (Phase 8): unavailable-state release page, typed-but-empty release metadata |
| `/privacy` | **Real content** (Phase 9): privacy position with evidence labels |
| `/security` | **Real content** (Phase 9): security controls, boundaries, and limits |
| `/about` | **Real content** (Phase 10): product explanation, principles, development status |
| `/faq` | **Real content** (Phase 10): 22 questions in five categories, native disclosures |
| `/support` | **Real content** (Phase 10): support status, troubleshooting, report checklist |

**No placeholder pages remain.** Every route now carries real content, and none
asserts a capability, certification, statistic, release, or contact channel that
does not exist. `PageNotice` is still used on About, Support, and Privacy, but as
a disclosure rather than as a "content coming later" placeholder.

### Hero section

`src/components/sections/HeroSection.tsx` replaces the Phase 1 introductory
placeholder. Copy:

- **Eyebrow:** `LOCAL AI · WINDOWS DESKTOP`
- **H1:** `Your AI coding workspace. On your terms.`
- **Lead:** Nexora is a Windows desktop AI coding assistant built around local
  models, giving you a more private, locally run coding workflow after the
  model is downloaded.

**No call-to-action button.** No installer exists, so a download button would
be a fake control. Instead the hero carries a non-interactive status line —
`In active development. The Windows installer is not available yet.` — and no
absolute-privacy claim, benchmark, or availability date.

**Typography-led, no illustration.** The refinement removed the right-hand
brand illustration along with the lower section, so the home page is a single
centred column of type: eyebrow, two-line headline lockup, lead, status line.
The composition is carried by the headline size, the measure of each line, and
the space around them. The two sentences of the headline each occupy their own
line (locked with `white-space: nowrap` only from 72rem, where the column is
provably wide enough) so the break does not depend on which font resolves —
the stack falls back from Segoe UI to Arial or Open Sans, and a natural wrap
lands on three lines in all of them.

Layout: the hero fills the space between header and footer and centres its copy
in it (`flex: 1 0 auto` on the hero inside a column `main`), so the composition
holds on both a 760px and a 1400px-tall viewport. At 1440×1000 the whole home
page is exactly one screen tall.

### Landmarks and heading order

`SiteLayout` renders the chrome once for every route: skip link → `header` →
`nav[aria-label="Primary"]`, `main#main` containing the routed page, then
`footer` → `nav[aria-label="Privacy and security"]`. No page duplicates that
markup; pages render only their own content. Because the footer no longer
mirrors the navigation, the current route is marked with `aria-current="page"`
exactly once per page, in the header.

Each page has **exactly one `h1`**, supplied by `PageIntro` (or the hero on the
home page). The skip link targets `main#main`, which carries `tabIndex={-1}`.

On a client-side route change, `SiteLayout` resets scroll to the top and moves
focus to `main`, so keyboard and screen-reader users land at the start of the
new page instead of on the link they just left. Focus is not moved on the
initial load, where it would steal focus from the address bar or a deep link.

Document titles are set per route through `useDocumentTitle` — for example
`Features — Nexora`. Home uses `Nexora — AI Coding Assistant`.

### Header composition

The header is a three-column grid at 64rem and above: brand in track 1,
navigation centred in track 2, an empty `minmax(0, 1fr)` in track 3. The
symmetric flexible tracks are what make the centring real — the navigation is
centred on the page, not offset by however wide the wordmark happens to be —
and they keep the brand and the links in separate tracks so they cannot collide
at intermediate desktop widths. Measured: 0px off centre from 1024px to 1920px,
with at least 69px of clear space between the wordmark and the first link at the
narrowest desktop width.

Links are `--text-sm`, medium weight, with a hairline underline on hover and on
the current page (plus `aria-current="page"`), so state is never carried by
colour alone. The 8px `li + li` prose margin from `base.css` is explicitly reset
on both header lists — without that reset the first label sits 4px out of line
with the rest, which is visible at 100% zoom.

### Footer content

Two stacked rows: brand and one sentence, then a hairline rule with the
`/privacy` and `/security` links beneath it — a compact closing line aligned to
the same left gutter as the wordmark, 44 px tall per link on touch and 24 px or
more with a fine pointer. There is no status paragraph (the former "under active
development / separate project" note was removed; it restated what the header,
the home hero, and `/download` already say), no navigation copy, no column
headings, no social accounts, no contact details, no copyright line, and no
release claims — none of those exist to state.

### Mobile navigation

The menu is an **ARIA disclosure**, not a modal dialog: the toggle owns
`aria-expanded` and `aria-controls`, focus stays in document order, and there
is no focus trap. The toggle is a native `<button type="button">` with a
visually-hidden "Menu" name and a drawn (not icon-font) glyph.

Implemented behaviour:

- opens/closes on toggle activation;
- closes when a link is selected;
- closes on `Escape` and returns focus to the toggle;
- closes on a pointer-down outside the header;
- closes automatically when the viewport crosses to desktop (≥ 64rem), so a
  stale open state can never leave a hidden panel open after a resize;
- event listeners are only attached while the menu is open.

Breakpoint: below **64rem** the desktop link list is replaced by the toggle and
panel; at 64rem and above the panel and toggle are display-hidden regardless of
state.

### Brand mark

`components/brand/ApertureMark.tsx` draws the aperture mark as inline vector
with `currentColor`, `aria-hidden`, and `focusable="false"`.

The repository's existing brand files (`resources/icons/nexora-icon.png`,
`apps/desktop/src-tauri/icons/*`) are **dark app-icon tiles**, which would sit
badly on a light page — and copying binary application assets into the website
is out of scope. The vector mark therefore re-draws the *same approved
geometry* locally: no binary asset is copied, and no network request is made.
If pixel-exact parity with an official exported brand file is required, that
file should be supplied by the owner and added under `src/assets/` in a later
phase.

## Phase 11 — animations and interactions

**Status: implemented** (approved **Phase 11 — Balanced Animations &
Interactions**). This section replaces the earlier "recorded requirement" note;
the constraints recorded there are all satisfied below, and nothing was copied
from any other site.

### Homepage particle background

`src/components/sections/ParticleField.tsx` renders one `<canvas>` behind the
hero copy on `/` only, inside a positioned `.home` wrapper (`pages/HomePage.css`).

**Why Canvas 2D.** The field needs ~20–130 moving elements, each with its own
velocity, opacity and soft radial falloff, redrawn every frame. As DOM nodes
that is 20–130 elements with a compositor layer each plus a style recalculation
per frame; on one canvas it is a single element and a single paint, and the
soft falloff is a pre-rendered sprite rather than a per-particle gradient. No
3D, no WebGL, no particle library, and **no new dependency** — the bundle grew
by roughly 1 kB gzipped.

**Palette and density.** Three colours, all taken from existing tokens:
`--color-border-strong` (the warm border tone, 45%), the `--color-accent` blue
(35%), and `--color-text-muted` (20%). Density is one particle per 9,000 CSS px²
clamped to 18–130, and coarse pointers (touch) get 60% of that, so a 1440×1000
desktop renders ~100 particles and a 390 px phone ~24. Depth comes from
per-particle radius (2.0–5.2 px), alpha and drift speed (5–15 px/s).

**Motion.** Each particle drifts in its own direction and wraps at the edges
with an 8 px margin, so motion never starts or stops abruptly and the field
cannot create overflow.

**Cursor interaction.** A pointer within a broad radius (18% of the field width,
clamped to 150–260 px) is attracted with a *quadratic* falloff — full strength at
the pointer, nothing at the rim. The essential detail is the relax term: every
particle's velocity also relaxes exponentially (1.4/s) back toward its own
drift, so particles sweep past the pointer and carry on. Without that term
attraction accumulates and the field collapses into a blob. Velocity is clamped
at 90 px/s, and the pointer state lives in refs — no React render is triggered
by a frame or a move.

Measured in the browser as motion per unit of ink (which normalises for the
extra particles attraction deposits near the pointer), one representative run:
**0.058 idle → 0.119 attracted (2.0×) → 0.054 settled (0.45× of attracted)**.
A region 430 px away stays at 1.04×, and one at the top-left at 0.59×. Local
density while the pointer is held stays at roughly 1.8–3.7× the canvas mean
across runs, i.e. the field remains distributed, and does not keep growing
(0.30 → 0.34 over four further seconds held).

**Reduced motion.** `prefers-reduced-motion: reduce` draws exactly one static
frame and never starts a loop; the pointer listener returns early, so moving the
pointer cannot restart motion. Selecting the preference at runtime is handled by
a `change` listener. The page is complete and legible with no animation at all.

**Lifecycle.** One `requestAnimationFrame` loop guarded by an idempotent
`start()` — Strict Mode's double-invoked effect cannot create a second loop
(verified: 42 frames / 0.7 s, not ~84). The loop stops on unmount (navigation
away), on `visibilitychange` when hidden (verified: 42 → 0 → 48 frames), and
under reduced motion; all listeners, the `ResizeObserver` and the frame handle
are released on unmount. Resize work is coalesced into one frame and does not
re-seed the field, so a resize does not visibly reset the particles. The backing
store is capped at 2× DPR.

**Accessibility and layering.** The layer is `aria-hidden="true"` with
`pointer-events: none` and `overflow: hidden`, sits at `z-index: 0` beneath a
`z-index: 1` hero, and contains nothing focusable. Links, text selection,
scrolling and focus order are untouched — verified by hit-testing, selection,
wheel scrolling, and the mobile disclosure.

**Contrast.** The hero's eyebrow and status line use `--color-text-muted`, which
is exactly 5.04:1 on white and therefore has almost no headroom. Solving the
worst case per colour, allowing up to three particle centres overlapping a pixel,
caps peak alpha at 0.219 / 0.076 / 0.090 for the three colours; the implemented
ranges (0.045–0.075, 0.016–0.025, 0.020–0.030) sit under those ceilings.
Presence comes from size and count, not darkness. Measured against the rendered
canvas while particles drift behind the text, the worst composite leaves the
12 px muted eyebrow at **4.76:1** and the status line at **4.86:1**, with the
headline at 16.3:1 and the lead at 7.0:1. Because a 12 px box can go seconds
without a particle crossing it, the sharper check is a bound: the darkest pixel
the field can currently produce composites to rgb(246), which still leaves muted
text at **4.68:1**. Every number above is above AA.

### Other motion (deliberately minimal)

- **Navigation states.** The active/hover mark is a drawn hairline (`::after`)
  that fades and grows with `--duration-base`/`--ease-out`, replacing an instant
  `text-decoration`. `aria-current="page"` still carries the state for assistive
  technology, so it is never colour-only.
- **FAQ disclosures.** The plus marker's vertical rule fades out via `opacity`
  instead of `display: none`, so opening an answer reads as a collapse rather
  than a blink. Still native `<details>`; no script.
- Existing hover transitions (links, buttons, footer links) were already
  token-driven and were left alone. No entrance animations, no scroll-driven
  reveals, no parallax, and no content hidden until an animation completes.

## Phase 12 — premium cursor and interaction system

**Status: implemented** (approved **Phase 12**). No dependency was added: the
whole phase is one SVG, CSS and a single pointer handler.

### The custom cursor

`src/components/motion/CustomCursor.tsx` draws one inline SVG glyph in a
`position: fixed` layer. It is the approved reference: a **hollow, rounded
cursor pointer** in the reference's own colour, `#192133` (now tokenised as
`--color-cursor`) — a blue-shifted navy, not black, which is the only accent the
glyph carries.

The geometry is **fitted, not eyeballed**. The reference was measured in its own
coordinate space: the three edges were least-squares fitted to the reference's
silhouette and intersected to recover the sharp vertices (−6.5, −5.9),
(71.1, 21.5) and (22.0, 74.7), giving interior angles of 51°, 67° and 62°. The
corner radii were then solved per corner — the tip and bottom corners are round
(11 and 7 units) while the right point is nearly sharp (4.5) — against a stroke
of 8.2 units. The glyph is authored in that coordinate space (`viewBox="0 0 68
68"`) and scaled to 26 px, so no resampling error is introduced. Comparing the
rendered result to the reference at the same size, the silhouettes match; the
reference is a small, soft JPEG, which limits how exact a comparison is
possible.

**The glyph is hollow.** A translucent interior was implemented first and then
removed: over the black display headings it read as a grey smudge rather than
as the reference. Because a dark navy outline on black type is nearly invisible,
the glyph carries a **keyline** — the same two contours stroked in
`--color-bg`. On the site's white and warm-white surfaces it is invisible, so the
pointer looks exactly like the reference; over text it opens a 1 px gap that
keeps the pointer legible. That is a hairline separation, not a glow or a drop
shadow.

**Positioning.** The hotspot is the **tip**, not the box corner: the tip sits
0.84 / 1.49 units inside the glyph's bounding box, and that offset is applied to
every move, so the drawn tip lands on the pointer (measured: 0.00 px error).
The offset is derived from the rendered glyph, so the stylesheet owns the size
and the two cannot drift apart.

**Motion.** The layer follows the pointer with a per-frame interpolation
(0.42 of the remaining distance), which is roughly two frames of catch-up —
smooth without feeling detached — and snaps instead of flying when the pointer
jumps more than 96 px. Position is written straight to `transform` from refs:
**no React state is touched by a pointer move**, and there is no per-frame
render. The loop **stops itself** once the cursor settles and restarts on the
next move, so an idle cursor costs nothing.

**States.** Hover scales the same glyph to 1.1; pressing settles it to 0.94.
The design never changes — no ring, no fill swap, no second cursor.

### Where the custom cursor applies, and where it must not

The native cursor is hidden only after the component has confirmed a **fine
pointer that can hover** and **no reduced-motion request**; the class it adds
(`html.has-nexora-cursor`) is also reverted under `prefers-reduced-motion` and
inside `@media (hover: none), (pointer: coarse)`, so the stylesheet alone can
never leave a pointerless page.

These keep their **native** cursor, and the decorative glyph hides over them, so
two cursors can never appear at once:

- `input`, `textarea`, `select`, `option`
- `[contenteditable]` that is actually editable
- anything with a CSS `resize` (resize handles)
- `:disabled` and `[aria-disabled="true"]` — the `not-allowed` state is the cue
- anything marked `[data-native-cursor]`

Everything else — including prose — shows the custom pointer. **Text selection
still works normally** (verified: 162 characters selected through the overlay),
but the I-beam *glyph* is replaced by the pointer over text. That is the
standard trade-off for a custom cursor and is the one place this phase departs
from strict native affordance; the form, editable and resize exemptions above
are where a missing specialised cursor would actually break something.

Leaving the window or losing focus hides the glyph and hands the native cursor
back; the next movement restores both.

### Buttons

`.btn` gains `transform` in its transition list and a pressed state that scales
to 0.985 (`--scale-press`). It is keyed on `:active`, never on hover, so a
hovered button never moves; it is excluded from `:disabled` and
`[aria-disabled="true"]`, so a disabled control cannot look pressed. Keyboard
activation produces the same `:active` state, and the existing focus ring is
untouched. Buttons do not grow or lift on hover — only colour and border shift,
as before.

Navigation links dim to 0.6 on press and keep their Phase 11 hairline underline;
they never translate, so the centred navigation cannot shift by a pixel.

### Navigation and the mobile menu

- **Menu icon.** Three bars that fold into a cross: the outer bars converge on
  the centre and rotate ±45°, the middle bar fades. Drawn as elements rather
  than swapping two SVG paths, because a path swap can only cut. The button's
  label now switches between "Menu" and "Close menu" for assistive technology,
  and `aria-expanded` still carries the state.
- **Panel.** Now always mounted and opened with `data-open`, so **both**
  opening and closing animate (a mounted/unmounted panel could only animate in).
  It is an **overlay** under the bar rather than a block inside it, so opening
  the menu no longer pushes the page down mid-transition. While closed it is
  `visibility: hidden` **and** `inert`, so it is unreachable by pointer, keyboard
  and assistive technology exactly as if it were absent. `visibility` flips at
  the end of the close transition and at the start of the open one.
- **Behaviour preserved:** Escape closes and returns focus to the toggle, focus
  stays in document order (no trap), a route change closes it (including
  back/forward), growing past the breakpoint closes it, `aria-controls` and
  `aria-expanded` are correct, and rapid toggling cannot desynchronise it.
- The brand logo now dims on hover; it previously had no response at all.

Because the panel stays mounted, the current-page marker exists twice in the
markup — once in the desktop list, once in the panel. Only one surface is
rendered at any viewport (`display: none` on the other, and `visibility: hidden`
plus `inert` while closed), and the browser suite asserts that **exactly one
visible** `aria-current="page"` link exists at both 390 px (once opened) and
1440 px.

### Page transitions

`.site__route` in `SiteLayout` wraps the routed content and is **keyed on the
pathname**, so a short 200 ms fade-and-rise (`--shift-enter: 6px`) replays on
every navigation. It is an **entrance only**: React Router puts the new page in
the DOM immediately and the animation starts from the stylesheet, so nothing is
withheld from the document, from assistive technology, or from search engines
while it plays (measured at 70 ms in: destination rendered, opacity ~0.4). A
browser that ignores the keyframes simply shows the base state. No transition
library, no exit animation, no duplicate content, no scroll restoration changes —
`SiteLayout` already resets scroll and moves focus to `main` on every route
change, and that still runs as before.

### Reduced motion

`prefers-reduced-motion: reduce` gives: no custom cursor at all (the native
cursor is never hidden), no route transition, no menu-icon or panel animation, no
FAQ reveal, and collapsed link/button transitions — the global rule in
`base.css` already floors every duration. Verified in the browser: 0 rAF
callbacks, `route-enter` absent, FAQ answer fully readable at opacity 1, content
complete, headline unchanged.

### FAQ

The disclosure marker already faded in Phase 11. Phase 12 adds a 200 ms
fade-and-rise on the answer as it opens (native `<details>` still does the work,
so the rule can only animate an element that is already open and readable).

---

## Phase 13 — glass navigation capsule and mobile polish

**Status: implemented** (approved **Phase 13**). Two goals only: one shared glass
navigation capsule, and mobile/tablet polish across the pages that already
exist. No page was redesigned, no section was added, no route changed, and no
dependency was added.

> **No design mockup was supplied for this phase.** The header was built from the
> written brief (one continuous capsule holding the brand and the navigation,
> restrained near-white glass, hairline border, soft shadow, full radius). Any
> resemblance to a specific reference image is a coincidence of that brief, not a
> reproduction of a supplied asset.

### The capsule

`SiteHeader` is still the single shared header — there is no per-page copy. Its
structure is now a transparent sticky band containing one glass surface:

```
.site-header                 sticky, transparent, no border, no bar
  .site-header__shell        container; positioning context for the panel
    .site-header__capsule    the glass: brand + nav (desktop) / brand + toggle
    .site-header__panel      the mobile menu card, a sibling that drops below it
```

The capsule is a single element with a `--radius-full` surface, one
`--border-width` hairline in `--color-glass-border` and `--shadow-glass`. Its
translucency is a token (`--color-glass`, `rgb(255 255 255 / 78%)`), not an
inline value, and it is applied behind an `@supports` test:

- with `backdrop-filter`: `blur(18px) saturate(1.5)` over the 78 % tint;
- without it: a 94 % white tint (`--color-glass-solid`) that carries the text on
  its own — no blur, no reliance on what is behind the header, same geometry.

Nothing about the surface is dark, neon, gradient-heavy, or more transparent
than legibility allows: body text on the glass measures the same colour as body
text on the page background, because the underlying tint is near-white.

**Desktop geometry.** From `64rem` the capsule shrinks to its content and centres
on the page (`width: fit-content; margin-inline: auto`). A content-sized pill
gives the brand and the links one shared gap and equal inner padding on both
sides; a stretched full-width pill left a dead column of glass to the right of
the last link. Measured: 808 px wide × 66 px tall at 1280/1440/1920, 736 px at
1024, 17 px of inner padding on each side, 24 px between the brand box and the
nav box, capsule centre exactly on the viewport centre at every width tested.

**Mobile geometry.** Below `64rem` the capsule is full-width inside the page
gutters — 350 px at 390, 280 px at 320 — 56 px tall, with the wordmark on the
left and the 44 × 44 toggle on the right. The toggle is borderless: it sits
*inside* the capsule, so an outlined box inside the shared surface would read as
a second box. Its bars still animate into a cross, and the panel is still a
sibling of the capsule that fades and rises into place (Phase 12 behaviour,
unchanged: `aria-expanded`, Escape, focus handling, close-on-route).

The panel lines up with the capsule's inline edges (`left/right:
var(--page-padding)` relative to the container's padding box), sits 8 px below
it, and gives every row a 44 px target.

**Header metrics are tokenised.** `--header-capsule-h`, `--header-block-pad` and
`--header-height` drive the capsule height, the band of transparent air around
it, `scroll-padding-top` in `base.css`, and the sticky legend offset in
`CapabilityGroup.css` — so an in-page anchor or a sticky label cannot end up
under the glass. At `48rem` and below the header is 72 px tall; above it, 90 px.

### What mobile polish changed

Everything here applies below the existing breakpoints; the desktop values are
untouched.

| Area | Change |
| --- | --- |
| Micro-labels | `--text-2xs` becomes `0.75rem` at `48rem` and below, so the uppercase labels that were 11 px are ≥12 px on phones. Desktop keeps 11 px. |
| Hero | New `max-width: 30rem` block: headline `clamp(2rem, 1.4rem + 3.4vw, 2.125rem)`, `line-height: 1.06`, balanced wrapping, and a smaller, left-aligned availability notice. The headline now breaks 2 + 1 from 320 px to 768 px instead of 3 + 2. |
| Footer | Mark row and legal links are 44 px targets by default and compact only under `(hover: hover) and (pointer: fine)` — capability, not width, because a 1024 px tablet is still a finger and a narrow desktop window is still a mouse. The compact mark row keeps its 20 px box and extends its hit area to 28 px, so desktop layout does not move. |
| FAQ | Related links at the end of an answer are 44 px targets on touch; with a fine pointer they stay one line box tall (24 px) instead of shrinking to a 16 px text run. `<details>` semantics, the reveal, and the summary's 52 px target are unchanged. |
| Long-form pages | Privacy/Security/About/Support keep their existing type scale; the readability work is the notice/label sizes above and a scroll offset that matches the taller header. |
| Download | Untouched in content: the "installer not available" notice is still explicit, and there is still no button, version, size, checksum, or date. |

No new section, card, badge, or interaction was introduced, and no page gained
or lost a heading.

### Two real defects found and fixed

1. **The custom cursor could paint in the top-left corner before the pointer
   moved.** `pointerover` fires at (0, 0) when the pointer enters the window, and
   the layer was made visible before any position had been applied. The layer is
   now shown only after a successful placement (`if (placed)`), and the move
   handler shows it explicitly after positioning.
2. **The first Tab skipped the skip link on a fresh load.** `SiteLayout` guarded
   its scroll/focus effect with a one-shot "first render" ref, but React
   `StrictMode` invokes effects twice on mount, so the discarded pass consumed
   the guard and the second pass focused `main` — moving the sequential focus
   point past the header. The effect now compares the previous pathname instead,
   which is idempotent under a double invoke. Verified: the first Tab on a fresh
   page focuses the skip link again.

### Particles: measured, deliberately unchanged

The Phase 11 field was measured before anything was decided, and it did not need
to change: the count is already scaled by canvas area, the backing store is
capped at DPR 2, and exactly one `requestAnimationFrame` loop runs.

| Context | Canvas | Coverage | Alpha | Loops |
| --- | --- | --- | --- | --- |
| Phone 390 × 844 @1× | 390 × 480 | 3.23 % | 0.05 | 0.97 |
| Tablet 768 × 721 @2× | 1536 × 1442 | 3.33 % | 0.07 | 0.97 |
| Desktop 1440 | 1440 × 679 | 3.43 % | 0.09 | 0.97 |

No emitter, trail, or second loop was added, the field is still independent of
the cursor, and `prefers-reduced-motion` still stops it entirely (0 rAF
callbacks in 500 ms). **No FPS claim is made**: frame rate was not profiled, and
headless Chromium is not a device measurement.

### Verification for this phase

- `pnpm typecheck` — no errors. `pnpm build` — success
  (`index-DEOl5w8h.css` 44.27 kB / 7.50 kB gzip, `index-C771Xsjz.js` 356.10 kB /
  110.93 kB gzip).
- A purpose-built browser suite (`puppeteer-core` driving headless Chromium
  against the dev server): **79 / 79 assertions pass**, covering capsule
  structure and geometry, glass tokens and the no-`@supports` fallback, brand
  sizing, nav and toggle targets, the hamburger-to-close animation, panel
  alignment and rows, menu open/close/Escape/close-on-route, reduced motion,
  touch behaviour (no custom cursor, native scroll, text selection), keyboard
  reachability of the skip link and its focus ring, FAQ disclosure behaviour, and
  the particle measurements above. React mounting was asserted per route (header
  present, one `h1`, zero console/page errors) — an HTTP 200 alone was not
  treated as proof.
- A viewport sweep at **320, 360, 375, 390, 412, 430, 600, 768, 1024, 1280, 1440
  and 1920** across all ten routes: **no horizontal overflow, no element outside
  the viewport, no clipped text, no target under 24 px once inline-prose links
  are excluded** (those are exempt under WCAG 2.5.8).
- **Desktop regression**: full-page captures before and after the phase at 1280
  and 1440. Below the header, every inner page is **pixel-identical** once the
  header's own growth is compensated (0 differing pixels at a 27 px offset);
  hero typography, measure, particle canvas size, cursor stacking, and container
  width are unchanged. The header band itself grew from 65 px to 90 px, which
  moves page content down by 25 px, and on viewports taller than the home
  content the hero (`.hero { flex: 1 0 auto }` inside a `100vh` column)
  re-centres about 13 px lower. Both are direct consequences of the approved
  capsule and were left as-is rather than compensated with arbitrary offsets.

**Not verified, and not claimed:** any physical device, iOS Safari, Firefox,
Safari, Windows, or a touch screen. Everything above was measured in headless
Chromium 153 on Linux with synthetic touch and pointer emulation, at DPR 1
(and DPR 2 for one tablet capture). Font metrics come from a system-sans
fallback, so exact line breaks may differ where the bundled font is available.
Landscape phone and safe-area insets were handled in CSS but not exercised on a
device with a notch.

---

## Phase 14 — SEO and discoverability

**Status: implemented** (approved **Phase 14**). Scope was technical SEO only:
per-route metadata, a configurable indexing policy, `robots.txt`, a sitemap that
is emitted only when it can be correct, and a semantics audit. No visual change
was made, no page was added, no copy was rewritten for keywords, and no
dependency was added.

> **No production domain exists in this repository.** Nothing in the code,
> configuration, or history names one. The phase therefore does **not** invent
> one: absolute URLs are configuration, and the committed default emits none.

### Where the metadata lives

| File | Responsibility |
| --- | --- |
| `src/config/metadata.ts` | One `title` + `description` per route, keyed by `RoutePath`. Missing a route is a type error. |
| `src/config/site-config.ts` | Pure resolution of the domain and indexing policy — shared by the browser bundle and the build. |
| `src/config/site.ts` | Reads that policy from `import.meta.env` for the app. |
| `src/hooks/usePageMetadata.ts` | Writes the head: title, description, robots, Open Graph, Twitter, canonical. Upserts by tag identity, never appends. |
| `vite/seo-plugin.ts` | Emits `robots.txt`, an optional `sitemap.xml`, and the indexing directive in `index.html`. |

`useDocumentTitle` is gone: it set only the title, which is why every route used
to inherit the single description in `index.html`. Each page now calls
`usePageMetadata(ROUTES.…)`, and the fallback route calls `usePageMetadata(null)`.

### Metadata by route

| Route | `<title>` | Description (one sentence, ≤ 160 chars) |
| --- | --- | --- |
| `/` | Nexora — AI Coding Assistant | Nexora is a Windows desktop AI coding assistant built around local models, designed for a more private, locally run coding workflow. In active development. |
| `/features` | Features — Nexora | What exists in the current Nexora implementation: a local-model coding workflow with workspace controls, described with an evidence label on every claim. |
| `/how-it-works` | How It Works — Nexora | The intended Nexora workflow, from preparing a local model to a coding session in the desktop client, including what has not been verified yet. |
| `/download` | Windows Download — Nexora | The Windows release of Nexora is being prepared. No installer is available yet; this page explains what will be published and what has to be ready first. |
| `/privacy` | Privacy — Nexora | An informational overview, not a legal policy: what the Nexora source code does with your code and conversations, what it does not do, and what is unverified. |
| `/security` | Security — Nexora | What the Nexora source code enforces to protect your machine, what is only designed, and what nobody has tested. Not an audit, not a guarantee. |
| `/about` | About — Nexora | About the Nexora project: a Windows desktop AI coding assistant designed around local models, how the pieces fit together, and what has been verified. |
| `/faq` | Frequently Asked Questions — Nexora | Answers about Nexora, checked against the project source: local models, privacy, security, downloads and support, including what is unknown or not built. |
| `/support` | Support — Nexora | What you can check yourself when Nexora misbehaves, what to record if something goes wrong, and why there is no public support channel yet. |
| `*` (fallback) | Page not found — Nexora | The address you requested is not part of the Nexora website. It may have been mistyped, or the page may not have been built yet. |

Every title is unique and ends with the brand; every description is unique and
describes the page rather than the product in general. No description claims the
installer exists, promises a support channel, or repeats a security guarantee.

### The production domain: configuration, not a guess

Two public variables (see `.env.example`) decide everything:

| Variable | Meaning |
| --- | --- |
| `VITE_SITE_URL` | The confirmed production origin, e.g. `https://<domain>`. |
| `VITE_SITE_INDEXABLE` | `true` to allow indexing. Only honoured together with a valid origin. |

`VITE_SITE_URL` is rejected — with a build-log warning that disables canonical
URLs and the sitemap — when it is not `https`, contains credentials, has a query
or fragment, carries a sub-path (the site is served from the root), is an IP
address, or points at `localhost`, a loopback address, `.local`, `.internal`, or
a `*.e2b.app` preview host. A trailing slash is normalised away.

**How to establish the domain later**, in full:

1. Confirm the domain and that it will serve the site from its root.
2. Put `VITE_SITE_URL=https://<domain>` and `VITE_SITE_INDEXABLE=true` in
   `website/.env` (git-ignored) or in the host's environment.
3. Rebuild. The build then emits `sitemap.xml`, a permissive `robots.txt`
   pointing at it, `index, follow` in the served HTML, and per-route
   `<link rel="canonical">` plus `og:url` once React mounts.
4. Configure any CDN or host to serve `dist/robots.txt` and `dist/sitemap.xml`
   from the root and to rewrite unknown paths to `index.html` (see "Hosting
   requirement" above). Nothing in this phase adds a host-specific file.

| Stage | `VITE_SITE_URL` | `VITE_SITE_INDEXABLE` | Result |
| --- | --- | --- | --- |
| Committed default / preview | unset | unset | `noindex, nofollow`; `robots.txt` = `Disallow: /`; no canonical, no `og:url`, no sitemap |
| Origin known, still private | set | unset | `noindex, nofollow`; no sitemap (canonicals are harmless but pointless) |
| Launch | set | `true` | `index, follow`; sitemap; canonical + `og:url` per route |

There is no way to reach the "launch" row by accident: the flag alone, without a
valid origin, is ignored and logged.

### Indexing policy, and what it is not

- Unconfigured builds ship `<meta name="robots" content="noindex, nofollow">` in
  `index.html` **before** JavaScript runs, plus `Disallow: /` in `robots.txt`.
  Both are needed: a crawler that does not render JavaScript only reads the
  second one, and the page tag covers the case where `robots.txt` is unreachable.
- The fallback route is never indexable, in any configuration
  (`noindex, follow`, so its outbound links are still followed). It is also
  never canonicalised.
- `robots.txt` has exactly one `User-agent: *` group, so there is no
  contradictory allow/disallow pair inside the file, and the file always agrees
  with the page-level directive because both come from `resolveSeoConfig`.
- **A robots directive is crawler guidance, not access control.** It does not
  make a deployment private, and it cannot stop a determined crawler or a direct
  fetch. Host-level authentication is the only real control.
- A genuine `404` status for the fallback route still requires host
  configuration; this phase did not add it, and the site still returns `200`
  with the not-found page rendered client-side.

### Sitemap

`dist/sitemap.xml` is generated from `SITEMAP_PATHS` — the nine content routes,
home first, filtered by each entry's own `indexable` flag — and only when both
configuration values are valid. It contains `<loc>` elements and nothing else:
`lastmod`, `changefreq` and `priority` are all optional in the protocol and every
value for them here would have been invented. The fallback route cannot appear
in it, because it has no `RoutePath`.

Validated: well-formed XML (Python `xml.dom.minidom`), correct
`http://www.sitemaps.org/schemas/sitemap/0.9` namespace, nine absolute URLs, all
of them real routes, no localhost or preview host, no date or frequency claims.

**Observed, and worth knowing before launch:** with no sitemap emitted, the dev
server answers `/sitemap.xml` with `404`, while `vite preview` answers `200` with
the HTML shell — because its SPA fallback rewrites every unknown path. That is
the same fallback the site needs for deep links, so a host must be configured to
exclude `/robots.txt`, `/sitemap.xml` and `/assets/*` from it; otherwise a
crawler asking for a (correctly) absent sitemap receives an HTML page. No host
configuration file was added in this phase, because the platform is not
established.

**Remaining prerequisite:** a confirmed production origin. Until then the file is
deliberately absent (a `404` on `/sitemap.xml`) rather than published with
invented URLs.

### Social metadata, and the image gap

`og:title`, `og:description`, `og:type` (`website`), `og:site_name`, and
`twitter:card` / `twitter:title` / `twitter:description` are written per route
and mirror the page. `og:url` is written only when a canonical exists.

**No `og:image` / `twitter:image` is emitted, and the card type is `summary`.**
The repository has no social preview asset (1200×630, brand-consistent), and a
screenshot of the site is not one — publishing a stale desktop capture as a
product preview would misrepresent the page. `site.webmanifest`, favicons, and
`apple-touch-icon` are equally absent, so nothing links to a file that does not
exist. This is a documented gap, not an oversight: adding a real asset is a
design task, not an SEO one.

### Structured data: deliberately omitted

No JSON-LD is included, and that is a decision rather than an omission:

- `Organization` and `WebSite` both want a `url`, and no canonical URL exists
  to point at;
- `Organization` also wants a `logo`, and there is no public raster logo asset —
  only a React `ApertureMark` component;
- `SoftwareApplication` would describe the desktop product, and its useful
  fields (version, download URL, price, rating) are exactly the facts that do
  not exist yet. The site's honest position is that the installer is unavailable;
  a rich-result payload implying otherwise would contradict `/download`.

When the domain and a real logo exist, `WebSite` + `Organization` become
addable from `src/config/metadata.ts` without changing any page. Adding them now
would have meant inventing at least one fact.

### Semantics

Audited, unchanged where already correct: one `h1` per route, no skipped heading
levels, one `header` / `main` / `footer` plus two named `nav` landmarks
("Primary", "Privacy and security"), `lang="en"`, no link without an accessible
name, no vague "click here" text, decorative SVG hidden from assistive
technology, and every internal `href` pointing at a route that exists. Nothing
was added to the markup, because the existing structure already satisfied the
audit.

### Architecture limits, stated plainly

This is a client-rendered SPA. Metadata is written after React mounts, so:

- crawlers that execute JavaScript (Google, Bing) see the per-route tags;
- crawlers that do not (most social unfurlers, many smaller search bots) see the
  `index.html` defaults, which describe the home page only;
- per-route social previews therefore **cannot** be correct without prerendering
  or SSR, which is deferred rather than claimed.

Nothing here guarantees indexing, ranking, or rich results. No search-engine
submission, `sitemap` ping, or verification token was performed or invented.

### Verification for this phase

- `pnpm typecheck` — 0 errors. `pnpm build` — success, and the emitted crawl
  files were inspected byte-for-byte.
- **Configuration unit checks (33):** accepted/rejected `VITE_SITE_URL` forms
  (http, localhost, loopback, `0.0.0.0`, IP, `[::1]`, `*.e2b.app`, sub-path,
  query, credentials, port, uppercase host, trailing slash) and flag spellings;
  both partial configurations resolve to the closed policy.
- **Browser suite, default (committed) policy — 214/214 assertions:**
  per-route title/description uniqueness, og/twitter mirroring, absence of
  canonical + `og:url` without a domain, absence of JSON-LD, exactly one of each
  owned tag, in-app navigation and back/forward updating the head with no
  duplicates or stale values, not-found non-indexable and non-canonicalised,
  zero console errors, and Phase 11–13 design checks (capsule 808×66 centred,
  72 px hero, particle canvas, cursor, footer).
- **Browser suite, configured policy — 57/57 assertions:** canonical and
  `og:url` correct on all nine routes, `index, follow`, not-found still
  `noindex, follow` and never canonicalised, canonical rewritten on navigation,
  `robots.txt` allowing crawling and referencing the sitemap, and a nine-URL
  sitemap with no fallback route and no invented dates.
- **Semantics audit — 91/91 assertions** across all ten routes.
- **Static checks:** deep links return `200`; `/robots.txt` is `text/plain`
  with one `User-agent` group; `/sitemap.xml` is `404` in the default build and
  valid XML in the configured one.

**Not verified:** any real search engine, social unfurler, or indexing service —
none was contacted and no ranking or indexing claim is made. Lighthouse/PageSpeed
was not run. Headless Chromium 153 on Linux with synthetic input is the only
browser used.

---

## Phase 15 — performance and accessibility

**Status: implemented** (approved **Phase 15**). The phase was audit-first: the
site was measured before anything changed, and only two defects justified a code
change. Nothing was optimised to produce a better-looking number, and the design
is byte-for-byte the same visually.

### What was measured, and with what

Everything below was measured in headless Chromium 153 on Linux through the
DevTools protocol, against the **production build** served by `vite preview`,
with the same harness before and after the change (the numbers are deltas from
CDP's `Performance` domain and from instrumented `getBoundingClientRect`,
`clientWidth`/`clientHeight`, `requestAnimationFrame` and `addEventListener`
calls, not estimates).

**Not available in this environment, and therefore not claimed:** Lighthouse,
PageSpeed Insights, Chrome UX Report, real Core Web Vitals field data, real
devices, and any GPU or low-power-device profiling. No FPS figure, no
Lighthouse score, and no "improvement percentage" is quoted anywhere in this
document.

### Baseline, and what actually needed changing

| Measurement | Baseline | After | Verdict |
| --- | --- | --- | --- |
| Initial load, JS | 1 file, 112.2 kB transferred | same | Fine: one request, gzip 111.5 kB |
| Initial load, CSS | 1 file, 7.8 kB transferred | same | Fine |
| First contentful paint | 108 ms | 100 ms (noise) | Fine |
| Layout shift (CLS) | **0.0000**, zero shift entries | 0.0000 | No work needed |
| Long tasks during load and 9 navigations | **none** | none | No work needed |
| Web fonts / images requested | **none** (system sans stack) | none | No font-shift risk to fix |
| Idle, 2 s on home | 28 ms script, 120 rAF, ~3.1 MB heap, ~202 nodes | 29 ms, 120 rAF | One loop, ~1.5 % of the main thread |
| Scroll 20 steps, home | 17 `getBoundingClientRect`, 34 `client*` reads, 59 rAF | **0, 0, 41** | Fixed (see below) |
| 25 pointer moves | 27.6 ms script, 6.4 ms style | 16.2 ms, 5.5 ms | Follows from the same fix |
| 9 in-app navigations | median 15 ms, max 23 ms, 8 layouts | median 14 ms, max 25 ms | No routing change justified |

The honest reading of the baseline: **the website had no significant
performance problem.** Layout shift was already zero, there were no long tasks,
no webfonts and no images, and the only continuously running work was the
single particle loop on the home page. The two changes below are the only places
where a measurement showed avoidable work.

### Change 1 — the particle field no longer treats scrolling as a resize

`ParticleField` scheduled its `resize()` path from a `scroll` listener as well
as from `resize` and a `ResizeObserver`. `resize()` reads two layout properties,
re-measures the canvas box and can reallocate the backing store. Measurement
showed 20 scroll steps produced 17 `getBoundingClientRect` calls, 34 `client*`
reads and 17 extra animation frames — with **zero** reallocations, because the
guard worked and the element's size never actually changed.

Only the *cached viewport position* needs invalidating on scroll — the pointer
handler already re-measures it lazily. So `scroll` now sets that flag and nothing
else, while `resize` (which can change size and DPR) still schedules a real
resize, and the `ResizeObserver` still covers every layout-driven size change.

Result: 0 layout reads and 0 extra frames per scroll step; the next pointer move
after a scroll measures exactly once. Verified behaviourally, including that the
field still animates at ~60 rAF/s and that a hidden tab still stops it.

### Change 2 — the custom cursor no longer disappears in forced-colours mode

A real accessibility defect, found only by emulating `forced-colors: active`.
The Phase 12 glyph is filled with the fixed `--color-cursor` navy, which the
forced-colours palette does not remap. On a forced-dark background the glyph
measured **1.31:1** against black — and because the component also suppresses
the native cursor, high-contrast users were left with **no usable pointer**.

`CustomCursor.css` now drops the decorative overlay and restores the native
cursor under `@media (forced-colors: active)`. Verified in both forced-light and
forced-dark: the layer is `display: none`, body and paragraph cursors are
`auto`, links are `pointer`, and outside that mode nothing changed
(`display: block`, `cursor: none`, glyph visible).

That media query is the phase's only intended visual change, and it only takes
effect when the operating system asks for forced colours.

### Deliberately not changed, with the measurements behind each decision

| Candidate | Decision and evidence |
| --- | --- |
| Route-level code splitting | **Not done.** React DOM is 56 % of the minified bundle (205 kB of 366 kB) and cannot be split; the nine pages total ~90 kB. Lazy routes would save roughly 6 % gzip on first load, add a loading state to every navigation (14–16 ms today), and would delay the Phase 14 per-route metadata update until each chunk resolves — a real SEO/UX regression for a 6 % win. |
| Removing unused CSS/JS primitives (`Surface`, `.btn--lg`, `.container--wide`, `.measure`) | **Not done.** Together they are ~1.2 kB raw / ~0.3 kB gzip, and they are part of the documented Phase 3 primitive layer. Removing them is an architecture decision, not a performance one. |
| Pausing particles when the hero leaves the viewport | **Not done.** Measured: the home page scrolls at most ~330 px, and its hero is ~690 px tall, so the canvas is never fully off screen. An `IntersectionObserver` would never fire; it would be pure complexity. |
| Reducing the particle count / lowering DPR further | **Not done.** Phase 13 measured 2.9–3.4 % ink coverage and ≤0.09 alpha at DPR ≤ 2 with one loop and ~29 ms of script per two seconds. There is no measurement suggesting it is heavy. |
| `content-visibility` / CSS containment | **Not done.** No layout cost was measurable: 0 layouts and 0 style recalculations during a 2 s scripted scroll, and 8 layouts across 9 route changes. |
| Preloading / splitting the stylesheet | **Not done.** One CSS file, 7.8 kB, one request. Splitting it would add requests without reducing critical CSS. |

### Accessibility audit

Measured, not eyeballed:

| Check | Result |
| --- | --- |
| Contrast of every rendered text/background pair | **1,349 pairs measured** at 1440, 390 and 320 px, including the open mobile panel, compositing translucent layers properly. **0 failures.** Lowest ratio **4.79:1** (12 px `cap-legend__title` on `#faf9f7`, needs 4.5:1). |
| Focus indicators | 22 Tab stops per route: every focused element draws the 2 px `--color-focus` ring (5.4:1 on white, 6.2:1 on the glass capsule); none lands on a zero-size element, none is nameless, none leaves the viewport. |
| Focus not obscured (WCAG 2.2 SC 2.4.11) | 45 Tab stops on `/privacy`: **0** content elements hidden under the sticky header; horizontal scroll-padding already accounts for it. |
| Skip link | First Tab stop on every route, 178 × 42 px when focused, moves focus to `main`, and the next Tab continues inside the page. |
| Mobile menu keyboard operation | Enter/Space opens, `aria-expanded` and the accessible name flip to "Close menu", Tab reaches all 8 rows, focus is **not** trapped (it continues to the footer), Escape closes and returns focus to the toggle, and the closed panel is `inert`. |
| Touch targets | No standalone control under 24 × 24 px at 320 or 390 px; touch-first sizing keeps 44 px rows. Inline prose links are exempt (WCAG 2.5.8). |
| Reflow / zoom (SC 1.4.10) | 320 px and 640 px (the 400 % equivalent): 0 px horizontal overflow, 0 clipped blocks. |
| Text spacing (SC 1.4.12) | Enlarged line height, letter and word spacing on `/security`: no overflow, no clipping. |
| Reduced motion | 0 rAF callbacks in 600 ms, no custom cursor, no route transition, particles drawn as one static texture, menu still opens instantly. |
| Landmarks, headings, link names | Re-verified on all 10 routes (Phase 14 suite): one `h1`, no skipped levels, one `header`/`main`/`footer`, two named `nav`s, no unnamed or vague links. |
| Forced colours | Defect found and fixed (Change 2 above). |

**Not verified, and not claimed:** WCAG 2.2 AA conformance as a whole. The checks
above cover the machine-checkable parts of contrast, focus visibility, target
size, reflow, text spacing, motion and semantics; they do not cover
screen-reader output (no AT was run), cognitive load, or the judgement calls in
SC 1.3.x and 2.4.x. No assistive technology was used.

### Verification for this phase

| Check | Result |
| --- | --- |
| `pnpm typecheck` | **0 errors** |
| `pnpm build` | **OK** — `index-GWKP8PZu.css` 43.95 kB (7.47 kB gzip), `index-CUZtObyo.js` 358.85 kB (111.86 kB gzip), `robots.txt` 0.75 kB |
| Build determinism | Two consecutive builds produce **identical sha256** for every emitted file |
| Bundle delta | JS **+19 B raw / +24 B gzip**, CSS **+113 B raw / +20 B gzip** — the forced-colours rule and the split scroll handler. No dependency added. |
| Phase 15 regression suite (`p15-regression.mjs`) | **143/143** on the production build *and* **143/143** on the dev server (where React StrictMode double-invokes effects) |
| Phase 14 metadata suite, re-run | **214/214** |
| Phase 14 semantics suite, re-run | **91/91** |
| Phase 15 contrast sweep | **1,349 pairs, 0 failures** |
| Phase 15 keyboard/targets/zoom suite | **88/88** |
| Responsive sweep | 9 widths (320, 360, 390, 430, 768, 1024, 1280, 1440, 1920) × 10 routes: 0 overflow, 0 clipped text, 0 headings behind the header |
| Browser console / page errors | 0 on every route, in every suite |

### Remaining limitations

- No real device, GPU, low-power mode, iOS Safari, Firefox or Windows browser
  was available; everything above is headless Chromium on Linux. The
  `backdrop-filter` glass capsule is the one component whose cost is likely to
  differ most on a real GPU, and that was not measured.
- No field data: no CrUX, no real-user metrics, no Lighthouse. "Performance" here
  means main-thread work, layout stability and request count in one synthetic
  browser.
- The particle loop still paints continuously on the home page (~29 ms of script
  per two seconds, ~1.5 % of one core). That is the approved design, it stops
  when the tab is hidden, and it never runs on any other route.

---

## Phase 16 — final verification and fixes

**Status: implemented** (approved **Phase 16**). This phase was verification and
corrective maintenance only: no redesign, no new page, component, effect or
dependency, and no copy restyling. Exactly **one** product defect was found and
fixed, plus its regression coverage.

### The defect

`GET /sitemap.xml` answered **`200 text/html`** — the single-page app shell —
while `robots.txt` says `Disallow: /` and no sitemap exists. Reproduction (before
the fix, production build via `vite preview`):

```text
$ curl -s -o /dev/null -w "%{http_code} %{content_type}\n" http://127.0.0.1:4173/sitemap.xml
200 text/html
$ curl -s http://127.0.0.1:4173/robots.txt | grep -c Sitemap
0
```

So a crawler that was explicitly told not to crawl was handed an HTML document
at the one URL a sitemap is expected at, and a monitoring check would have read
it as "sitemap present". The Phase 14 `configureServer` hook only covered the dev
server; `vite preview` (and any static host with an SPA fallback) fell through to
the app shell.

### The fix

`vite/seo-plugin.ts` now has one shared resolver, `resolveCrawlFile(config, url)`,
used by both `configureServer` and a new `configurePreviewServer`. With no
confirmed production origin the preview server answers:

```text
$ curl -s -i http://127.0.0.1:4173/sitemap.xml | head -3
HTTP/1.1 404 Not Found
Content-Type: text/plain; charset=utf-8

No sitemap: no confirmed production domain is configured.
```

`robots.txt` is unchanged in content and still served from the same
configuration. When `VITE_SITE_URL` and `VITE_SITE_INDEXABLE` *are* set, the same
resolver emits the real `sitemap.xml` from the route table — unchanged behaviour.
`(req as { url?: string }).url` is used because this tsconfig does not load
`@types/node`, so the Connect request type is `IncomingMessage` without `url`.

**Still a deployment-host concern, and not pretended otherwise:** a static host
that rewrites every unknown path to `index.html` will *still* return the shell for
`/sitemap.xml` unless that host is told to exclude it (or the built `sitemap.xml`
is simply present, which it is once a domain is configured). Nothing here fixes a
host; the local preview now behaves correctly, and the host-side rule is written
down in the plugin.

### Verification

| Check | Result |
| --- | --- |
| `pnpm typecheck` | **0 errors** |
| `pnpm build` | **OK** — byte-identical to the Phase 15 baseline: `index-CUZtObyo.js` 358,848 B (111,514 B gzip), `index-GWKP8PZu.css` 43,946 B (7,423 B gzip) |
| Phase 14 metadata suite, assertion tightened | **215/215** (was 214/214; the old assertion accepted `404 *or* an HTML shell`, it now requires `404 text/plain` with an explanatory body, and a second assertion was added for that body) |
| Phase 14 semantics suite | **91/91** |
| Phase 15 regression suite | **143/143** |
| Phase 15 keyboard / targets / zoom / text-spacing suite | **88/88** |
| Phase 15 contrast sweep | **1,349 pairs, 0 failures**, lowest 4.79:1 |
| Phase 16 route audit (10 routes) | 0 console errors, 0 page errors, 0 failed requests, 0 duplicate ids/titles/descriptions, 0 dead links, one `h1` each, all routes `noindex, nofollow` |
| axe-core 4.14.0 (harness-installed, **not** a project dependency) | 20 runs (10 routes × 1440 px and 390 px, plus the open mobile menu): **0 violations, 0 errors** |
| Edge-case URLs | `/faq/`, `/FAQ`, `/faq?x=1`, `/faq#main` → FAQ; `/features/extra`, `/%2Ffaq`, `/index.html` → not found; `/faq/../privacy` → `/privacy`; reload keeps route-specific title, description and `robots`; in-app navigation resets scroll to `0` and moves focus to `MAIN` |
| Sticky header vs. content headings | 30 sampled headings across 1920/1440/1024 px: **0** hidden behind the header; `scroll-padding-top` already handles anchors |
| Secrets / local paths in shipped assets | none — no tokens, no `VITE_SITE_*` values, no `/home/`, `/Users/`, or drive paths, no fabricated domain |

### Limitations of this phase

- `axe-core`'s `color-contrast` rule is reported **incomplete**, never passing,
  on this design (118 nodes per sweep) because it cannot resolve contrast over
  gradient and `backdrop-filter` backgrounds. Contrast is therefore verified by
  the project's own rendered-pair measurement (1,349 pairs), not by axe, and no
  claim of full WCAG conformance is made.
- No real device, screen reader, iOS Safari, Firefox or Windows browser was
  available; headless Chromium on Linux with synthetic input is the only browser
  used, and no assistive-technology behaviour is claimed.
- No Lighthouse, PageSpeed Insights, CrUX or field data exists here, and none is
  quoted.
- The suites are external harnesses (`puppeteer-core` + `axe-core` in `/tmp`),
  not repository tests: this project still has **no test runner and no test
  dependency**, and adding one was out of scope for a corrective phase.
- The site remains undeployed, unindexed, and without a confirmed domain.

---

## Phase 17 — deployment and public-launch preparation

Preparation only: **nothing was deployed, published, or indexed**, and no
hosting account, DNS record, certificate, or production credential was created.
The launch checklist is `DEPLOYMENT.md`; this section records what changed in
the code and why.

### Three launch blockers found by auditing a production-shaped server

Each was reproduced against a locally running server with `curl -I`, fixed, and
covered by a regression test.

| # | Defect | Why it blocked launch | Fix |
| --- | --- | --- | --- |
| 1 | `X-Robots-Tag: noindex, nofollow` was sent on **every** response, including public pages and assets | It contradicts `robots.txt: Allow: /` and would have deindexed the site on the day indexing was enabled | Moved to `NOINDEX_HEADERS` (`server/http.mjs`) and applied only to `/admin` and `/api/*` |
| 2 | `HEAD /api/portal/status` returned `404` while `GET` returned `200` | Uptime monitors and load-balancer health checks use `HEAD`; the portal would have looked permanently down | The API router now maps `HEAD` onto the matching `GET` handler; Node suppresses the body |
| 3 | Hashed Vite assets (`index-BbfPWENj.js`) did not match the immutable-cache pattern, which expected a dot before the hash | Every asset was sent `no-cache`, so a CDN or browser revalidated on every request — pointless work, worse cold loads | Corrected the pattern in `serveFile()`; hashed names are now `public, max-age=31536000, immutable` |

### Added: production-only security headers

`productionSecurityHeaders(config)` in `server/http.mjs` returns
`Strict-Transport-Security`, `X-Frame-Options: DENY` and
`Content-Security-Policy: frame-ancestors 'none'` **only** when
`NEXORA_PORTAL_ENV=production` with secure cookies, and `{}` otherwise. They are
applied once per request in `server/index.mjs`, so static pages, API responses
and error responses all carry them. Tying them to configuration rather than
making them constant keeps the development server and the embedded sandbox
preview working (plain HTTP, framed) while a production deployment is hardened.
HSTS is deliberately not sent from a development server.

### Verified, not assumed

| Check | Result |
| --- | --- |
| `pnpm test:portal` | **73/73** (60 existing + 13 new in `server/tests/deployment.test.mjs`) |
| `pnpm typecheck` / `pnpm build` | 0 errors / unchanged hashes, `index-BbfPWENj.js` 384.94 kB (118.94 kB gzip), `index-CCuSGdpx.css` 57.36 kB (9.07 kB gzip) unchanged |
| Indexable build (`VITE_SITE_URL` + `VITE_SITE_INDEXABLE`) | `robots.txt` = `Allow: /` + `Sitemap:`; `sitemap.xml` = exactly the 9 public URLs, `/admin` appears **0** times; `index.html` = `index, follow` |
| Unindexed build | `robots.txt` = `Disallow: /`; `/sitemap.xml` = `404 text/plain` explaining that no domain is configured (never the SPA shell) |
| Production-shaped run (temp env file, `NEXORA_PORTAL_ENV=production`) | HSTS + `X-Frame-Options: DENY` + `frame-ancestors 'none'` on pages and API; session cookie `HttpOnly; Secure; SameSite=Strict`; a foreign `Origin` is refused with `403`; `HEAD /api/portal/status` 200 |
| Development run | no HSTS/XFO/CSP; `/admin` still `noindex`; public pages indexable per the build |
| Path traversal / private files (`/../`, `/%2e%2e/`, `/.env`, `/.portal-data/portal.json`, `/server/index.mjs`) | no file contents returned |
| Existing suites after the change | portal browser **72/72**, Phase 16 route audit 10 routes (0 console errors, 0 dead links), configured-policy suite **57/57**, axe **0 violations / 20 runs**, portal contrast 198 pairs 0 failures, Phase 15 regression **143/143**, keyboard **88/88**, site contrast **1,349 pairs 0 failures**, Phase 14 metadata **215/215**, semantics **91/91** |
| Production start with missing/invalid configuration | exits non-zero and names only the variable; covered by a test that spawns the real server |
| `SIGTERM` | listener closed, exit code 0 |

Not performed, and not claimed: any test on a real hosting provider, real TLS
termination, a real domain, a real device, or a screen reader; no Lighthouse or
field-performance measurement; nothing was deployed, published, or indexed.

## Developer Account Portal (`/admin`)

**Status: implemented locally, verified, and not deployed.** This is the
pre-launch portal task, not a numbered website phase; the roadmap is not
renumbered and Phase 17 has not been started.

`/admin` is a private sign-in page and, once authenticated, a Developer
Dashboard for the Owner / Super Admin account. The public pages are unchanged
and continue to pass their own suites. The full architecture — threat model,
session rules, provisioning, production requirements, and limitations — is in
**[`DEVELOPER_PORTAL.md`](./DEVELOPER_PORTAL.md)**; this section is the summary.

### What was added

- `server/` — a **zero-dependency Node server** (`node:http` only) that serves
  the built site and `/api/portal/*` from one origin. Passwords use
  `node:crypto`'s scrypt (N=32768, r=8, p=1, per-hash random salt); sessions are
  32 random bytes delivered in an `HttpOnly; SameSite=Strict` cookie and stored
  only as `sha256`; state lives in a single JSON file outside the web root.
- `server/provision-owner.mjs` — the **only** way an account can exist. Offline
  CLI, password read from a hidden prompt or stdin (never `argv`, never logged),
  refuses to create a second account, `--rotate-password` also revokes sessions.
- `server/env-file.mjs` + `server/owner-config.mjs` — server-side `.env` loading
  and first-run Owner provisioning from `website/.env`
  (`NEXORA_PORTAL_OWNER_EMAIL` / `_NAME` / `_PASSWORD`). Startup only *creates*
  an owner that does not exist; a restart never overwrites a password, and
  rotation is the explicit `--from-env --rotate-password` command. Only key names
  are ever logged.
- `src/portal/` — the sign-in page, the dashboard shell, and the five panels
  (overview, website status, content, support, settings), all built from the
  site's existing tokens and primitives.
- Route/config integration: `ROUTES.admin`, `PAGE_METADATA[ROUTES.admin]` with
  `indexable: false` and `private: true`, so `/admin` is `noindex, nofollow` and
  **cannot** enter `SITEMAP_PATHS`. No navigation link to it anywhere.
- `vite.config.ts` proxies `/api` to the portal in dev and preview, keeping the
  browser's `Host` so the origin check sees one origin.
- `website/.env.example` documents the server-only variables; `.gitignore` ignores
  `.portal-data/`.

### Security posture, in one paragraph

Authentication, authorisation, session expiry, rotation, and sign-out are
server-side; the React layer is presentation only. Every mutating or private
request is origin-checked *and* CSRF-checked with a session-bound token; there is
no CORS surface at all; failed sign-ins are one generic message with a
constant-time path for unknown accounts; attempts are rate limited per account
and per address; sign-in rotates the session; a password rotation kills every
session. There is no public registration and no setup URL, so a stranger cannot
claim the owner account. A missing or corrupt store answers `503`; it never
falls back to "authenticated".

### Verification for this task

| Check | Result |
| --- | --- |
| `pnpm typecheck` | **0 errors** |
| `pnpm build` | OK — no new dependencies; JS 384.94 kB (118.94 kB gzip), CSS 57.36 kB (9.07 kB gzip), `robots.txt` 0.75 kB |
| `pnpm test:portal` (new, `node --test`, no deps) | **73/73** (37 unit + 23 integration + 13 deployment-contract) |
| Portal browser suite (headless Chromium, dev server + real proxy) | **72/72** — sign-in validation, generic errors, show/hide, sign-in, sections, refresh, sign-out, keyboard, reduced motion, 9 widths, no overflow/clipping |
| axe-core on sign-in and dashboard | **0 violations** |
| Rendered contrast on portal pages | **198 pairs, 0 failures**, lowest 5.04:1 |
| Phase 14 metadata suite | **215/215** |
| Phase 14 semantics suite | **91/91** |
| Phase 15 regression suite | **143/143** |
| Phase 15 keyboard/targets suite | **88/88** |
| Phase 15 contrast sweep (public site) | **1,349 pairs, 0 failures** |
| Phase 16 route audit (10 public routes) | 0 console errors, 0 dead links, 0 duplicate titles/descriptions |
| Configured build (`VITE_SITE_URL` + indexable) | sitemap has **exactly the 9 public URLs**; `/admin` appears **0** times |
| `.env` provisioning, end to end (real server process) | first start creates the owner and sign-in returns `200`; restart leaves the hash untouched; `--from-env --rotate-password` rotates and revokes sessions; invalid config provisions nobody, logs variable names only, and still serves the site |
| Credential leakage | local `.env` password absent from `dist/index.html`, every `dist/assets/*` file, API responses, and the server's stdout/stderr |

### Not done, and not claimed

Nothing is deployed, published, committed, or indexed. There is no production
domain, no TLS, no host configuration, and no identity provider — the portal has
never run outside this development environment. There is one account model
(Owner) with no second factor, no email delivery, no recovery email flow, and a
single-process store. Those limits are listed in `DEVELOPER_PORTAL.md` §13.


## Current implementation status

**Phases 1-13 complete, plus the separate-page architecture correction and a
pre-Phase 11 UI refinement.**

Present in this directory:

- a valid HTML entry point (`index.html`) titled
  `Nexora — AI Coding Assistant`;
- a TypeScript/React entry point (`src/main.tsx`) wrapped in `BrowserRouter`;
- a route table (`src/App.tsx`) covering all nine routed pages plus a
  not-found fallback, sharing one `SiteLayout`;
- the token / base / utility stylesheet layers described above;
- five reusable UI primitives in `src/components/ui/`;
- a sticky `SiteHeader` driven by `NavLink` (with `aria-current` and a real
  keyboard-operable mobile menu), rendered as the Phase 13 glass capsule, and a
  minimal `SiteFooter`;
- the Phase 4 `HeroSection` as the whole home page (the Phase 5 showcase was
  removed in the UI refinement), with the Phase 11 particle background behind it;
- the Phase 6 `FeaturesPage`: five evidence-backed capability groups;
- the Phase 7 `HowItWorksPage`: a five-step workflow with evidence labels;
- the Phase 8 `DownloadPage`: unavailable-state release page with typed release
  metadata and no download control;
- the Phase 9 `PrivacyPage` and `SecurityPage`: evidence-labelled trust content,
  with `FactList`, `TrustLabel`, `TrustLegend`, and `TrustSection` as shared
  presentation;
- the Phase 10 `AboutPage`, `FaqPage`, and `SupportPage`: product explanation,
  22 disclosed questions, and useful guidance with no invented contact channel;
- `src/config/routes.ts` as the single source of route and navigation truth;
- `src/config/metadata.ts` (titles and descriptions), `src/config/site.ts` +
  `src/config/site-config.ts` (the SEO policy), `src/hooks/usePageMetadata.ts`
  (the head writer), and `vite/seo-plugin.ts` (the generated `robots.txt`,
  optional `sitemap.xml`, and indexing directive);
- a private Developer Account Portal at `/admin` — sign-in page, dashboard, and
  the zero-dependency Node server in `server/` that authenticates and authorises
  it (not deployed; see `DEVELOPER_PORTAL.md`);
- Vite, TypeScript, and pnpm workspace configuration;
- a website-local `.gitignore`.

Verified: `pnpm typecheck` and `pnpm build` pass; all nine routes plus the
fallback render the correct page with exactly one `h1` each and no skipped
heading levels; `/download`
contains no button, no download CTA, no executable reference, and no version,
checksum, size, or date; `/privacy` and `/security` contain no invented contact
details, legal entity, effective date, audit claim, or certification; `/about`,
`/faq`, and `/support` contain no invented support channel, price, release date,
version number, or hardware specification; every
figure quoted elsewhere on the site matches the product source; and every
`var(--…)` reference resolves to a defined token.

**Delivered so far, in order:** the design foundation and page architecture
(Phases 1–10), the cursor-reactive home particle field (Phase 11), the premium
cursor and interaction system (Phase 12), the glass navigation capsule with
mobile/tablet polish (Phase 13), the technical SEO foundation — per-route
metadata, a configurable indexing policy, generated `robots.txt`, and a
sitemap that only exists once a domain is confirmed (Phase 14) — an
evidence-driven performance and accessibility pass (Phase 15), the private
developer portal with its environment-based owner provisioning, and deployment
preparation (Phase 17), whose launch checklist is `DEPLOYMENT.md`.

**Deferred by design:** all scroll animation, including anything
scroll-triggered; per-route prerendering or SSR (so search and social crawlers
that do not execute JavaScript see only the home page's metadata); a favicon; a
social preview image; host configuration and deployment. Phase numbers for the
remaining work are deliberately not restated here: this phase was delivered as
Phase 15, and renumbering the rest of the approved roadmap is not this
document's call.

**Not** done and **not** claimed: the website is not deployed, not hosted, not
submitted to any search engine, not indexed, and its rankings are not measured
or improved. It is not production-ready and not connected to any real Windows
installer. No installer URL, version, size, or checksum exists yet, and no
benchmarks, reviews, or compatibility guarantees are published.

## Scope rules for future website work

- Make website changes inside `website/` only.
- Do not modify the desktop application, its Rust/Tauri code, or its build
  configuration to accommodate the website.
- Do not add website dependencies to the root manifest.
- Do not commit secrets, tokens, private keys, or production environment files.
- Do not add telemetry or analytics.
- Keep the developer portal (`/admin`, `src/portal/`, `server/`) inside
  `website/`, and never expose server-only configuration as a `VITE_*` variable.
- Do not represent mockups as genuine screenshots, and do not publish
  capabilities that have not been verified.
