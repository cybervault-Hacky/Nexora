# 06 — Professional Website Builder Architecture

**Status:** Proposed for owner approval  
**Related:** [05-coding-agent-workflow.md](05-coding-agent-workflow.md), [04-ai-runtime-and-model-strategy.md](04-ai-runtime-and-model-strategy.md), [07-security-and-privacy.md](07-security-and-privacy.md)

---

## 1. Purpose and boundaries

Nexora must generate complete, professional web projects — not isolated snippets. It must choose an appropriate stack for each brief, scaffold a real project, implement responsive layouts and interactions, run it locally, verify it, and report honestly.

### 1.1 The four distinct outcomes

| Outcome | Meaning | Evidence required | What Nexora may say |
|---------|---------|-------------------|---------------------|
| **Generated** | Files exist on disk | File writes recorded | "Generated 14 files" |
| **Built** | The production build completed | Build command exit 0, output exists | "Build passed" |
| **Tested / previewed** | The app ran locally and checks passed | Dev server ready, HTTP response, console capture, screenshots | "Preview loads; 4 checks passed" |
| **Deployed** | Published to a hosting provider | Deployment API response and public URL check | Not available at 1.0 |

These outcomes are never conflated. A generated site is not a tested site; a local preview is not a deployment.

### 1.2 Scope at 1.0

**In scope:** static multi-page sites, marketing sites, portfolios, blogs/docs, React single-page apps, Next.js full-stack apps with local SQLite, animation-rich sites, code-based 3D experiences, responsive design, accessibility checks, local preview, build/lint/test workflows, export.

**Out of scope:** production deployment, cloud provisioning, payment integration, complex authentication, native mobile apps, e-commerce backends, CMS hosting, image/video generation, drag-and-drop visual editing.

---

## 2. Project brief

A structured brief is captured before generation. Free text is parsed into the same schema; missing fields use explicit defaults shown to the user.

```json
{
  "name": "Acme Studio",
  "kind": "marketing | portfolio | blog | docs | web_app | dashboard | fullstack | 3d_experience",
  "goal": "Win freelance clients for a design studio",
  "audience": "Startup founders",
  "pages": ["home", "work", "about", "contact"],
  "content": {"source": "user_provided | generated", "notes": "..."},
  "brand": {
    "tone": "calm, editorial, premium",
    "colors": ["#0E1116", "#F4F2EC", "#8FA7FF"],
    "fonts": "system or bundled OFL fonts",
    "references": ["..."]
  },
  "features": ["contact form (local only)", "case study grid", "scroll animation", "3D hero"],
  "backend": {"needed": false, "data": "none", "auth": false},
  "constraints": {"no_cdn": true, "offline_assets": true, "browsers": "modern"},
  "deployment_target": "static | node | unspecified"
}
```

**Brief rules:**

- Ask at most five questions; infer the rest and state assumptions.
- "Generated content" must be clearly labelled and easy to replace; no fake client testimonials presented as real.
- If the user supplies brand assets, use them; otherwise generate a restrained placeholder identity and list it as a limitation.
- If the user asks for a backend but no data model, ask one focused question rather than inventing a schema.

---

## 3. Stack selection

### 3.1 Decision matrix

| Brief | Recommended stack | Why | Avoid |
|-------|-------------------|-----|-------|
| Static marketing site, 3–10 pages, SEO | **Astro + TypeScript** (or plain HTML/CSS/JS for a tiny one-pager) | Minimal JS, fast static output, content collections, easy hosting | Next.js SSR for a brochure site |
| Single-page interactive landing with custom animation/3D | **Vite + TypeScript + Three.js/R3F** | Full control, fast dev server, no framework overhead | Heavy CMS stack |
| Web app/dashboard with client state | **React + Vite + TypeScript + Tailwind + Radix** | Mature component model, accessible primitives, fast iteration | Astro for a stateful app |
| SEO + server rendering + API + database | **Next.js (App Router) + TypeScript + SQLite/Prisma** | SSR/SSG, route handlers, one project | Static-only stack that cannot meet the data requirement |
| Blog/docs with content | **Astro content collections** or **Next.js MDX** | Typed content, static output, good defaults | Hand-rolled markdown parser |
| 3D product configurator/experience | **React Three Fiber + Drei + Three.js** (Vite or Next) | Declarative scene graph, ecosystem, React integration | Raw WebGL unless the scene is trivial |
| Animation-heavy marketing | **Astro or Vite + Motion + CSS** | Compositor-friendly, accessible | Scroll-jacking libraries, unlicensed plugins |
| Offline/no-install demo | **Static HTML/CSS/JS** | Runs from disk with zero dependencies | Any stack requiring a package install |

### 3.2 Selection rules

1. Choose the **lightest stack that satisfies the brief**.
2. Do not default every project to Next.js, React, or Tailwind; explain the choice in the plan.
3. TypeScript is the default for application code. Plain JavaScript is acceptable for a small static page where types add no value.
4. Prefer the project's existing stack when editing an existing repository; never migrate frameworks as part of an unrelated task.
5. If the brief is ambiguous about backend needs, default to static and state the assumption.
6. If the user explicitly requests a stack, honour it unless it cannot meet the requirement; explain the conflict.
7. Pin dependency versions and preserve lockfiles.

### 3.3 Default design stack

- **Styling:** CSS variables + modern CSS (grid, container queries, `clamp()`); Tailwind CSS where it improves consistency and the project already uses it. Avoid runtime CSS-in-JS.
- **Components:** accessible primitives (Radix) for interactive UI; custom components for brand expression.
- **Motion:** Motion (MIT) for UI/entrance, CSS transitions for simple states, IntersectionObserver for reveal; GSAP only after licence review and only if the brief requires its specific capabilities.
- **3D:** Three.js + React Three Fiber + Drei (all MIT).
- **Icons:** Lucide (ISC) or custom SVG; no icon fonts.
- **Fonts:** self-hosted OFL fonts (Inter, JetBrains Mono, or the user's licensed font) or a system stack; no remote font CDN by default.
- **Forms:** local validation; no external form endpoint unless requested and approved.
- **Database:** SQLite for local full-stack demos; Prisma or better-sqlite3; migrations included.

---

## 4. Bundled templates

Templates make generation reliable and offline-capable. They are ordinary, readable projects, not proprietary formats.

| Template | Stack | Includes | Default use |
|----------|-------|----------|-------------|
| `static-basic` | HTML/CSS/JS | Semantic layout, tokens, responsive nav, no build step | Tiny sites, offline demos |
| `astro-marketing` | Astro + TS | Layout, SEO, sitemap, content collections, 6 page stubs | Multi-page marketing/portfolio |
| `astro-blog` | Astro + TS | Posts, tags, RSS, syntax highlighting | Blogs/docs |
| `vite-react-app` | React + Vite + TS | Router, layout, error boundary, test setup | SPAs/dashboards |
| `vite-react-three` | React + Vite + TS + R3F | Canvas, scene, loader, fallback, performance guard | 3D experiences |
| `next-fullstack` | Next.js + TS + SQLite | App Router, route handler, schema, seed, env example | Full-stack apps |
| `landing-motion` | Vite + TS + Motion | Hero, scroll reveals, reduced motion | Animated landing pages |
| `blank` | None | Empty folder + conventions | User-controlled builds |

**Template requirements:**

- Builds and previews on a clean machine after `install` (or with no install for static).
- No secrets, no analytics, no remote trackers.
- Responsive at 360–1440 px; accessible landmarks, focus states, labels.
- Pinned dependencies; lockfile committed; licence manifest.
- A `README.md` with commands, structure, and known limitations.
- A `DESIGN.md` describing tokens and layout decisions.
- A test hook (`npm test` or a documented smoke check) even if minimal.
- Versioned with the app; template updates do not silently change existing projects.

**Template quality gate:** each template is tested in CI for build, preview, responsive screenshots, and axe violations before release.

---

## 5. Generation pipeline

```
Brief
  → Stack decision + rationale
  → Plan (pages, components, data, assets, checks)
  → Scaffold template (offline)
  → Design tokens (colour, type, spacing, radius, motion)
  → Content structure and copy
  → Pages and components
  → Interactions and animation
  → 3D scene (if requested)
  → Assets and fonts
  → Install dependencies (network, approved)
  → Build
  → Preview + browser checks
  → Bounded repair
  → Verification report + export instructions
```

### 5.1 Design system first

Before generating pages, the agent writes a small token file:

```css
:root {
  --bg: #0e1116;
  --surface: #151a20;
  --text: #f4f2ec;
  --muted: #a8afb9;
  --accent: #8fa7ff;
  --radius-sm: 8px;
  --radius-md: 16px;
  --space-1: 4px;
  --space-4: 16px;
  --space-8: 32px;
  --font-sans: "Inter", system-ui, sans-serif;
  --font-mono: "JetBrains Mono", monospace;
  --measure: 68ch;
}
```

Rules:

- Two font families maximum; one accent colour; a coherent radius and spacing scale.
- Colour contrast checked before it is used for text.
- Tokens are used consistently; no scattered hex values.
- The design must match the brief, not Nexora's own brand. Nexora's periwinkle is not the default for client projects.

### 5.2 Content quality

- Write real, specific copy from the brief; no lorem ipsum unless the user asks for placeholders.
- Avoid fake statistics, fake testimonials, and fake client logos presented as real.
- Label generated placeholder content in `CONTENT_NOTES.md`.
- Structure headings hierarchically (one `h1` per page); write meaningful link text; add alt text for meaningful images and empty alt for decorative ones.

### 5.3 Anti-generic design rules

Reject the default "centered hero + gradient + three feature cards" unless the brief calls for it. Prefer:

- a clear typographic hierarchy and an editorial grid;
- one strong composition rather than many weak sections;
- restrained colour with one accent;
- intentional whitespace and alignment;
- real imagery or well-crafted abstract SVG/CSS, not stock-photo hotlinks;
- motion that supports hierarchy, not decoration;
- mobile layouts designed, not merely stacked.

### 5.4 Multi-page structure

- Shared layout with header/footer/navigation and skip link.
- Per-page metadata (title, description, canonical where appropriate).
- Consistent spacing and container widths.
- Active navigation state and keyboard-accessible mobile menu.
- 404 page for multi-page sites.
- Sitemap/robots only for static sites where appropriate; never reference a domain the user has not provided.

---

## 6. Responsive design

**Breakpoints:** 360 (small phone), 480 (large phone), 768 (tablet), 1024 (small laptop), 1280 (desktop), 1440+ (wide). Use `clamp()` for fluid type and spacing; container queries for components where supported.

**Requirements:**

- No horizontal overflow at any tested width.
- Touch targets ≥ 44×44 px on touch layouts.
- Navigation collapses accessibly with a visible focus trap and Escape to close.
- Images use `srcset`/`sizes` or responsive CSS; no fixed widths that overflow.
- Tables and code blocks scroll inside their own container, not the page.
- Text remains readable at 200% zoom and with increased line spacing.
- Reduced motion removes non-essential animation.
- Print styles for content pages where practical (optional).

**Verification:** screenshots at 360, 768, 1024, 1440; DOM check for `scrollWidth > clientWidth`; menu open/close; focus order.

---

## 7. Animation architecture

| Need | Default | Notes |
|------|---------|-------|
| UI state transitions | CSS transitions | 120–240 ms, transform/opacity |
| Entrance/reveal | Motion or IntersectionObserver | Stagger ≤ 5 items; no layout thrash |
| Scroll-linked | CSS scroll-driven animations where supported, fallback IntersectionObserver | No scroll-jacking; respect reduced motion |
| Complex timelines | Motion | Avoid heavy orchestration for simple pages |
| 3D | R3F `useFrame` with delta clamping | Pause when offscreen; cap DPR |
| Page transitions | Optional; only for SPAs | Must not delay content or break back/forward |

**Rules:**

- Animate `transform` and `opacity`; never animate `width`, `height`, `top`, or `box-shadow` in a loop.
- Use `will-change` sparingly and remove it after the animation.
- No infinite ambient animation on content pages unless explicitly requested; provide a pause.
- `prefers-reduced-motion: reduce` disables transforms, parallax, smooth scroll, and autoplay; content remains visible without animation.
- Motion must not hide content from crawlers or screen readers.
- Test with CPU throttling (4×) on a mid-range machine; report jank rather than claiming 60 fps.

---

## 8. Code-based 3D experiences

**Stack:** Three.js + React Three Fiber + Drei (MIT), or vanilla Three.js for simple scenes.

**Scene architecture:**

- A single canvas per page section, lazy-loaded below the fold.
- Scene graph split into assets, environment, camera rig, and interaction controllers.
- Disposal of geometries, materials, textures, and render targets on unmount.
- Asset pipeline: GLTF/GLB with Draco/Meshopt compression, KTX2 textures where appropriate.
- Loading state with progress; error boundary with fallback content.

**Performance budgets (starting points, verified per scene):**

| Budget | Target |
|--------|--------|
| Draw calls | < 100 |
| Triangles | < 500k |
| Texture size | ≤ 2048×2048 per map |
| Device pixel ratio | cap at 2 (1.5 on low-end) |
| Frame time | ≤ 16.7 ms on reference GPU, ≤ 33 ms on integrated |
| JS for 3D chunk | lazy-loaded; report size |

**Fallback:** detect WebGL2; if unavailable or context creation fails, show a static poster/hero image and a short explanation. Do not leave a blank canvas.

**Verification:** WebGL context exists, no fatal shader errors, canvas has non-zero size, screenshot is non-blank (or the fallback is shown), console clean, memory does not grow after route changes. Interactivity cannot be fully proven by a screenshot; the report says "scene renders" rather than "experience works".

---

## 9. Backend and database

**When needed:** forms that persist data, dashboards, content management, authentication explicitly requested.

**Default local stack:** Next.js route handlers + SQLite (Prisma or better-sqlite3) + Zod validation. For simple demos, a JSON file is acceptable if concurrency is not a concern, but state it.

**Rules:**

- Schema and migrations are committed; seed data is separate and clearly labelled.
- Validate all input server-side; never trust client validation.
- Parameterized queries only.
- Secrets live in `.env.local`, which is gitignored; provide `.env.example` with placeholders.
- Never expose secrets to the client bundle; no `NEXT_PUBLIC_*` secrets.
- Authentication and payments are out of scope at 1.0 unless explicitly requested; if requested, require a security review and use a maintained library rather than hand-rolled crypto.
- Database files are local; explain backup and reset commands.
- No production database provisioning or cloud credentials.

**Verification:** migration applies, seed runs, API route returns expected status and shape, invalid input returns 400, no secret appears in build output.

---

## 10. Assets, fonts, and licences

- All assets local; no hotlinking, no remote trackers, no CDN dependencies by default.
- Fonts: self-hosted WOFF2 with OFL/licence files; system stack fallback.
- Icons: Lucide (ISC) or custom SVG; record in `ASSETS.md`.
- Images: user-provided, generated placeholder SVGs/gradients, or permissively licensed sources with attribution; no scraped images.
- Optimize: compress images, use modern formats, set width/height to avoid layout shift, lazy-load below the fold.
- Record source, licence, and attribution for every third-party asset.
- Generated projects must not include Nexora branding unless requested.

---

## 11. Dependency management

- Detect and preserve the package manager (`package-lock.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lockb`).
- Add the minimum dependencies; prefer built-in platform features.
- Pin versions; no `latest`, no floating ranges added by the agent beyond the project's convention.
- Install with scripts disabled by default; request a scoped exception for packages that need postinstall binaries.
- Do not upgrade unrelated packages during a feature task.
- Record each added dependency with version, purpose, and licence.
- If a registry is unreachable, report the blocked install; do not vendor from an untrusted mirror.
- Optional: run a licence and vulnerability report as part of the final checks; report findings without auto-upgrading everything.

---

## 12. Preview and local runtime

- **Framework dev server** for HMR: Astro, Vite, Next, with explicit loopback host flags.
- **Static server** in Rust for `static-basic` or built output: no Node required, MIME types, directory index, path traversal protection.
- **Port allocation:** ephemeral free port, verified loopback listener, job-contained.
- **Readiness:** port + HTTP + framework ready line, 30 s timeout.
- **Console capture:** browser console via CDP, plus server stdout/stderr.
- **Device presets:** 390×844, 768×1024, 1024×768, 1440×900; custom width.
- **HMR caveat:** after agent edits, wait for HMR or reload before screenshots; capture a build hash to avoid testing stale code.
- **Stop:** kills the process tree; frees the port; updates the UI.

---

## 13. Verification and visual quality

### 13.1 Automated checks

| Check | Tool | Pass condition |
|-------|------|----------------|
| Typecheck | `tsc --noEmit` or framework check | exit 0 |
| Lint | project linter | exit 0 or no new errors |
| Build | `npm run build` | exit 0; output exists |
| Tests | `npm test` | exit 0; test count > 0 |
| Routes | HTTP GET for each route | 2xx/3xx; no 404 for declared pages |
| Console | CDP capture | no fatal errors; warnings summarized |
| Network | CDP capture | no failed local assets; external requests listed |
| Responsive | screenshots at 4 widths | no overflow; layout intact |
| Accessibility | axe-core | zero critical/serious, or documented exceptions |
| Links | local link check | no broken internal links |
| 3D | WebGL check + screenshot | context OK, non-blank or fallback |

### 13.2 Visual review

- Screenshots are stored with viewport, URL, timestamp, and build hash.
- If a baseline exists, compare with a perceptual diff; dynamic content is masked or flagged.
- The agent may describe what it sees, but "visually checked" requires captured evidence.
- Subjective quality (composition, typography, brand fit) is reviewed by the agent against the brief and, where possible, by the user. It is not a guarantee.

### 13.3 Quality rubric

| Dimension | Weight | Criteria |
|-----------|--------|----------|
| Visual design | 25 | Hierarchy, spacing, type, colour, composition, brand fit |
| Responsive | 15 | No overflow, usable at 360–1440, touch targets |
| Accessibility | 20 | Semantics, contrast, focus, labels, reduced motion, axe |
| Performance | 15 | Build size, image optimization, no obvious jank, lazy 3D |
| Code quality | 15 | Readable, typed, consistent, no dead code, no secrets |
| Content | 10 | Complete, specific, honest, no fake claims |

A project below 70/100 overall, or with a critical accessibility failure, is reported as "not yet professional" with specific gaps. The score is advisory and must not be presented as an objective certification.

---

## 14. Deployment boundary and export

- **No deployment at 1.0.** Nexora does not hold hosting credentials, publish to production, or claim a site is live.
- **Export:** the project folder is the deliverable. Provide a `README.md` with install/build/run commands, environment variables, and hosting notes for common static/Node hosts.
- **Optional post-1.0:** explicit, provider-specific deployment with separate permission, credential storage, preview-versus-production separation, and a verified public URL check. This requires a new security design and owner approval.
- **Language:** "ready to deploy" is allowed only when the build passes and the export is complete; "deployed" is never used without an actual deployment.

---

## 15. Failure handling

| Failure | Detection | Response |
|---------|-----------|----------|
| Install fails | non-zero exit, registry error | classify (network, version, peer dep, scripts); one repair; then ask |
| Build fails | non-zero exit, parsed error | fix smallest cause; max 3 attempts |
| Preview fails to start | no port/HTTP within timeout | show command and last 100 lines; try a different port once |
| Blank page | screenshot blank + console error | inspect error, fix; if no error, mark unverified |
| Hydration mismatch | console error | fix server/client divergence; do not suppress |
| Missing asset | 404 in CDP | add asset or fix path; no broken image |
| 3D context lost | WebGL event | restore or fallback; report |
| Font not loading | 404/FOUT | self-host or fallback; no remote CDN |
| Layout overflow | DOM measurement | fix at the offending breakpoint |
| Axe violation | axe-core | fix or document with user acceptance |

---

## 16. Example briefs and decisions

| Brief | Decision | Why |
|-------|----------|-----|
| "Portfolio for a photographer, 5 pages, fast, no backend" | Astro + static images + minimal JS | SEO, speed, simple hosting |
| "SaaS dashboard demo with charts and filters" | React + Vite + TS + a chart library | Client state, no SEO need |
| "Marketing site with a 3D product hero and a waitlist form" | Next.js or Astro + R3F; form via local API/SQLite | SEO + 3D + persistence |
| "One-page event site, works offline from a USB stick" | Static HTML/CSS/JS | No install, no server |
| "Blog with MDX and tags" | Astro content collections | Typed content, static output |
| "Internal tool with login" | Out of scope at 1.0; propose Next.js + a maintained auth library after security review | Auth requires a dedicated threat model |

---

## 17. Decisions and open questions

| ID | Decision | Recommendation | Status |
|----|----------|----------------|--------|
| D-29 | Stack selection | Lightest stack that meets the brief; explicit rationale | Proposed |
| D-30 | Default marketing stack | Astro + TS | Proposed |
| D-31 | Default app stack | React + Vite + TS | Proposed |
| D-32 | Default full-stack | Next.js + SQLite | Proposed |
| D-33 | 3D stack | Three.js + R3F + Drei | Proposed |
| D-34 | Templates | Bundled, versioned, tested offline scaffolds | Proposed |
| D-35 | Deployment | Out of scope at 1.0 | Proposed |
| D-36 | Static server | Rust server fallback for no-Node projects | Proposed |
| D-37 | Animation library | Motion + CSS; GSAP only after licence review | Proposed |

Open: Q-22 whether Tailwind is a default or opt-in; Q-23 whether a component library should be bundled; Q-24 maximum project size for generation; Q-25 whether to support Vue/Svelte stacks at 1.0 (currently React/Astro/Next only).

---

## 18. Sources

- Astro — https://astro.build/
- Vite — https://vite.dev/
- React — https://react.dev/
- Next.js — https://nextjs.org/
- Three.js — https://threejs.org/
- React Three Fiber — https://docs.pmnd.rs/react-three-fiber
- Drei — https://github.com/pmndrs/drei
- Motion — https://motion.dev/
- Radix UI — https://www.radix-ui.com/
- Tailwind CSS — https://tailwindcss.com/
- Lucide — https://lucide.dev/
- Playwright — https://playwright.dev/
- axe-core — https://github.com/dequelabs/axe-core
- Prisma — https://www.prisma.io/
- better-sqlite3 — https://github.com/WiseLibs/better-sqlite3
- WebGL — https://www.khronos.org/webgl/
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/

Access date: 2026-10-17. Licences must be re-verified before shipping generated templates.
