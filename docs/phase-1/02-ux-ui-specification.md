# 02 — UX/UI Design Specification

**Status:** Proposed for owner approval  
**Scope:** Windows desktop application, dark theme at 1.0, optional light theme post-1.0  
**Related:** [01-product-scope-and-requirements.md](01-product-scope-and-requirements.md), [03-application-architecture.md](03-application-architecture.md)

---

## 1. Design intent

Nexora should feel like a precision instrument for building software: calm, confident, and expensive without being loud. The interface borrows the conversational clarity of a great chat product, the material restraint of premium native software, and the depth of carefully used glass — but it must have its own identity. No proprietary layout, iconography, or brand expression is copied.

### Brand attributes

| Attribute | What it means in the UI | What it must never become |
|-----------|-------------------------|---------------------------|
| **Precise** | Aligned grids, tabular numbers, exact spacing, no accidental borders | Clinical, cramped, spreadsheet-like |
| **Calm** | Dark graphite canvas, low-saturation surfaces, one accent | Sleepy, low-contrast, lifeless |
| **Deep** | Layered panels, soft inner highlights, subtle translucency | Frosted-everything, blurry text, glass for its own sake |
| **Capable** | Clear status, evidence, diffs, commands, controls | Cluttered dashboards, developer-only jargon |
| **Human** | Plain-language errors, graceful recovery, no hype | Anthropomorphic mascots, fake enthusiasm, emoji chrome |
| **Distinct** | A recognizable mark, a periwinkle signature, a composer silhouette | Generic "AI purple gradient" or copied interface |

### Design thesis

> The conversation is the steering wheel; the project is the road; the evidence panel is the dashboard.

The user should always be able to answer three questions without opening a menu:

1. **What is Nexora doing right now?**
2. **What changed, and can I undo it?**
3. **What has actually been verified?**

---

## 2. Brand identity and application icon

### 2.1 Wordmark

- Lowercase **nexora** in Inter Display Semibold, tracking `-0.03em`, off-white.
- Optional compact form: the icon alone at 16–20 px in the title bar; the wordmark appears when width permits.
- The wordmark is never stretched, outlined, gradient-filled, or placed on busy imagery without a scrim.

### 2.2 Icon concept — "Nexus Mark"

A custom geometric mark, not an emoji, sparkle, brain, chip, or generic chat bubble. The concept is three converging facets that form an abstract **N** inside an incomplete rounded-square aperture. The open corner suggests an aperture or portal; the central meeting point is the "nexus". One facet carries the periwinkle accent; the others are off-white/graphite. The mark is legible in pure monochrome.

**Construction (master 256 × 256):**

```
┌──────────────────────────────┐  Canvas 256×256, transparent
│  ╭────────────────────────╮  │  Aperture ring: 192×192, radius 44,
│  │                        │  │  stroke 10, off-white @ 22%, gap 36
│  │      ██                │  │  at top-right (from 12 o'clock to 2 o'clock)
│  │       ███              │  │
│  │        ███             │  │  N left stem:  x 72–96,  y 76–180
│  │         ███            │  │  N diagonal:   from (96,76) to (160,180),
│  │          ███           │  │                 width 24, split into two tones
│  │           ███          │  │  N right stem: x 160–184, y 76–180
│  │            ██          │  │  Accent facet: lower-right diagonal segment
│  │                        │  │  Accent: #8FA7FF
│  ╰────────────────────────╯  │  Safe area: mark within 80% of canvas
└──────────────────────────────┘
```

**Rules:**

- The N is constructed from three parallelogram facets with 60° terminals; no rounded stroke caps except a 1 px optical softening at small sizes.
- The aperture ring is omitted below 24 px; the N remains.
- At 16 px, the mark uses a simplified two-tone silhouette with a minimum 2 px stem separation.
- The icon must pass a squint test at 16 px and a silhouette test in pure black.
- App icon background: graphite `#0E1116` with a very subtle vertical luminance shift (not a rainbow gradient). The mark may sit on transparency for title-bar use.
- The accent facet is the only colour in the icon. No glow, no drop shadow baked into the mark.

**Required exports (produced in a later phase, not now):**

| Asset | Sizes | Use |
|-------|-------|-----|
| SVG master | 256 viewBox, monochrome + accent variants | Source of truth |
| ICO | 16, 20, 24, 32, 40, 48, 64, 128, 256 | Windows exe, shortcuts, taskbar |
| PNG | 256, 512, 1024 | Store, docs, installer |
| Title-bar mark | 16, 20 | In-app chrome |
| Monochrome | 16–256 | High contrast, tray, print |
| Installer graphic | 164×314 BMP + 55×55 BMP | NSIS/WiX if required |

**Validation gates:** recognizability at 16 px; contrast ≥ 3:1 against both `#0E1116` and `#F4F2EC`; no text; no thin strokes below 1.5 px at 16 px; trademark clearance before final adoption (Q-12).

### 2.3 Voice

- Short, specific, non-anthropomorphic. Nexora "runs", "checks", "stops"; it does not "dream", "think magically", or "love".
- Status uses verbs and evidence: "Editing 3 files", "Build passed in 12.4 s", "Preview loads at 127.0.0.1:5173", "Unverified: screenshots unavailable".
- Never use "perfect", "guaranteed", "flawless", "superhuman", or "just works".
- Errors: what happened, why, what to do next. No raw stack traces in the primary message.

---

## 3. Visual language

### 3.1 Colour system (dark theme)

| Token | Value | Use |
|-------|-------|-----|
| `bg/base` | `#0A0C0F` | Window canvas, deepest layer |
| `bg/ambient` | radial `#151A22` at 60% top-left, transparent 70% | Subtle depth behind shell |
| `bg/panel` | `#111419` | Sidebar, docks, settings surface |
| `bg/elevated` | `#171B21` | Cards, popovers, menus |
| `bg/raised` | `#1D232B` | Hovered rows, active tabs |
| `border/subtle` | `rgba(255,255,255,0.07)` | Default separators |
| `border/strong` | `rgba(255,255,255,0.12)` | Focused/selected surfaces |
| `text/primary` | `#F4F2EC` | Headings, primary content |
| `text/secondary` | `#A8AFB9` | Labels, metadata |
| `text/tertiary` | `#737C88` | Placeholders, disabled labels |
| `text/inverse` | `#0A0C0F` | Text on accent fills |
| `accent/base` | `#8FA7FF` | Primary action, selection, focus |
| `accent/hover` | `#A7B8FF` | Hover |
| `accent/pressed` | `#738DF0` | Pressed |
| `accent/subtle` | `rgba(143,167,255,0.12)` | Selected rows, chips |
| `success` | `#63C98D` | Verified, passed |
| `warning` | `#E3B36A` | Unverified, caution |
| `danger` | `#E07575` | Failure, destructive |
| `info` | `#7CC7E8` | Neutral system information |
| `code/bg` | `#0D1014` | Code blocks, terminal |
| `code/text` | `#D7DEE8` | Code default |

**Rules:**

- Accent coverage target: **≤ 5% of visible pixels**. Selection, focus, one primary button, one active indicator.
- Semantic colours are used only for status, never decoration.
- No neon, no saturated purple-blue gradient washes, no rainbow syntax rain.
- Gradients are limited to: a faint ambient radial behind the shell, the icon's luminance shift, and progress shimmer. No text gradients.
- Colour is never the only signal: pair with icon, label, or pattern.

### 3.2 Glass and depth

| Token | Recipe | Allowed use |
|-------|--------|-------------|
| `glass/panel` | `rgba(17,20,25,0.68)` + `blur(20px) saturate(130%)` + 1 px `border/subtle` + inset top highlight `rgba(255,255,255,0.05)` | Composer, floating panels, command palette, popovers |
| `glass/raised` | `rgba(24,28,35,0.80)` + `blur(28px) saturate(140%)` + `border/strong` | Menus, model picker, dialogs |
| `glass/scrim` | `rgba(5,7,10,0.55)` + `blur(8px)` | Modal backdrop only |
| `shadow/low` | `0 1px 2px rgba(0,0,0,.35)` | Cards |
| `shadow/mid` | `0 8px 24px rgba(0,0,0,.32)` | Popovers |
| `shadow/high` | `0 24px 64px rgba(0,0,0,.45)` | Dialogs, composer focus |

**Hard constraints:**

1. Maximum **three** simultaneous blurred layers visible.
2. Never blur behind body copy; glass surfaces contain short labels or controls only.
3. Never nest glass inside glass.
4. Body text on glass must retain ≥ 4.5:1 contrast against the worst-case underlying content. If not, increase the panel opacity scrim.
5. Disable blur when `prefers-reduced-transparency: reduce`, when Windows "Transparency effects" is off, or when the GPU is under load / software rendering is detected. Fall back to `bg/elevated` at 96% opacity.
6. Blur is a material, not an animation. Do not animate `backdrop-filter`; animate opacity/transform of the surface instead.
7. Measure GPU cost on integrated graphics; if a panel causes frame drops, raise opacity before reducing readability.

### 3.3 Typography

| Style | Family | Size / line | Weight | Tracking | Use |
|-------|--------|-------------|--------|----------|-----|
| Display | Inter | 32 / 40 | 600 | −0.02em | Empty states, onboarding |
| H1 | Inter | 24 / 32 | 600 | −0.02em | Page titles |
| H2 | Inter | 20 / 28 | 600 | −0.01em | Section titles |
| H3 | Inter | 16 / 24 | 600 | 0 | Card titles |
| Body | Inter | 14 / 22 | 400 | 0 | Controls, labels |
| Chat | Inter | 15 / 26 | 400 | 0 | Conversation text |
| Small | Inter | 13 / 20 | 400 | 0 | Metadata |
| Caption | Inter | 12 / 16 | 500 | 0.01em | Chips, timestamps |
| Button | Inter | 13 / 16 | 500 | 0.005em | Buttons |
| Code | JetBrains Mono | 13 / 20 | 400 | 0 | Code, diffs |
| Terminal | JetBrains Mono | 12.5 / 19 | 400 | 0 | Terminal |

- **Families:** Inter (SIL OFL) and JetBrains Mono (SIL OFL), bundled locally as WOFF2 with `font-display: swap` and metric-adjusted fallbacks. No remote font requests.
- **Features:** `tnum` for counters and timings, `cv05`/`ss01` optional; no ligatures in code by default (user toggle).
- **Measure:** chat text max 72 characters (~760 px); settings text max 78 characters.
- **Text scaling:** respect Windows text scaling up to 200%; layout uses relative units and wraps rather than clipping.
- **No all-caps** except 11 px overline labels with +0.08em tracking, used sparingly.

### 3.4 Spacing, radii, and sizing

- **Spacing scale:** 2, 4, 8, 12, 16, 20, 24, 32, 40, 48, 64. Primary rhythm is 8 px; 4 px for dense controls.
- **Radii:** 4 (chips), 8 (inputs), 10 (buttons), 12 (cards), 16 (panels), 20 (modals), 24 (composer), 999 (pills).
- **Control heights:** 28 (compact), 32 (standard), 36 (primary), 40 (composer buttons). Minimum hit target 32×32; 40×40 for primary actions.
- **Icon sizes:** 14 (inline), 16 (default), 18 (toolbar), 20 (nav), 24 (empty states). Stroke 1.5 px, rounded joins, consistent optical weight.
- **Borders:** 1 px, never 2 px except focus rings. Inner highlights are 1 px inset, not outer glows.
- **Density:** comfortable by default; a compact density may be added post-1.0.

### 3.5 Iconography

- Custom 24 px grid icon set, stroke-based, 1.5 px, rounded caps, geometric. It may use an open-source base (Lucide, ISC) but must be curated and optically adjusted; no mixing libraries.
- Brand marks are filled; functional icons are stroked.
- No emoji as UI iconography. No coloured icon sets. Status icons use semantic colour plus shape.

### 3.6 Motion

| Token | Duration | Easing | Use |
|-------|----------|--------|-----|
| `motion/instant` | 80 ms | linear | Hover tint, press |
| `motion/fast` | 140 ms | `cubic-bezier(0.2,0.8,0.2,1)` | Chips, tooltips |
| `motion/base` | 200 ms | `cubic-bezier(0.2,0.8,0.2,1)` | Panels, menus |
| `motion/slow` | 280 ms | `cubic-bezier(0.2,0.8,0.2,1)` | Dialogs, onboarding |
| `motion/exit` | 120 ms | `cubic-bezier(0.4,0,1,1)` | Dismissals |

**Rules:**

- Animate only `transform` and `opacity`; never width, height, top, or `backdrop-filter`.
- No animation may block input. Streaming text is not animated per character.
- Message entrance: opacity 0→1 and translateY 6→0 over 180 ms; stagger ≤ 5 items at 20 ms.
- Composer focus: border/glow over 160 ms, no scale beyond 1.005.
- Send↔stop morph: 160 ms icon crossfade and border-radius shift.
- Streaming caret: 2 px accent bar, 1.2 s opacity pulse; disabled under reduced motion.
- Progress: determinate bars for downloads/checks; indeterminate shimmer only on a 2 px track, never on text.
- Panels: slide/fade 220 ms; content does not reflow mid-animation.
- **Reduced motion:** remove translation, scale, shimmer, and looping ambient motion; keep opacity transitions ≤ 120 ms; keep progress state changes instant. Honour both the OS setting and an in-app override.
- **Performance:** target 60 fps; no long task > 100 ms during transitions; use CSS transforms and compositor-friendly layers; cap blur layers.

---

## 4. Window architecture and layout

### 4.1 Window chrome

- Custom title bar, 40 px, with `-webkit-app-region: drag` regions and no-drag controls.
- Left: 16 px Nexora mark, wordmark when width ≥ 1100, project breadcrumb.
- Center: global command palette trigger with `Ctrl K` hint (subtle).
- Right: environment badge (Local), update indicator if available, and native Windows minimize/maximize/close controls (46×32 hit areas).
- Double-click drag region toggles maximize. Right-click opens the system menu. Snap Layouts must work on Windows 11 (hover maximize).
- The title bar uses `glass/panel` over the ambient background; it is 40 px tall and never contains body text.
- Window minimum: **960×640 logical px**; recommended 1440×900; supports 100–200% DPI without clipping.

### 4.2 Shell layout (≥ 1280 px)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  ◆ nexora   Acme Studio / website        ⌘ Search or run a command   — □ ×  │ 40
├───────────────┬──────────────────────────────────────────────┬───────────────┤
│  + New chat   │  Conversation                                │  TASK         │
│  Search       │                                              │  ● Editing    │
│               │  ┌ user ────────────────────────────────┐    │  Plan         │
│  PROJECTS     │  │ Build a 4-page studio site with…      │    │  ✓ Analyze    │
│  ● website    │  └───────────────────────────────────────┘    │  ✓ Plan       │
│  ○ dashboard  │                                              │  ● Edit 3 files│
│               │  Nexora                                      │  ○ Build      │
│  TODAY        │  I’ll scaffold an Astro site with…           │  ○ Preview    │
│  Chat…        │                                              │               │
│  Chat…        │  ┌ tool · read 4 files ──────────────┐       │  CHANGES (3)  │
│  YESTERDAY    │  └───────────────────────────────────┘       │  M index.astro│
│  Chat…        │                                              │  A hero.astro │
│               │──────────────────────────────────────────────│  M theme.css  │
│               │  ┌────────────────────────────────────────┐  │               │
│               │  │ +  Describe what you want to build…    │  │  CHECKS       │
│               │  │    [project] [model]          [ ➤ ]    │  │  ✓ Build      │
│               │  └────────────────────────────────────────┘  │  ○ Tests      │
├───────────────┴──────────────────────────────────────────────┴───────────────┤
│  TERMINAL · OUTPUT · PROBLEMS · PREVIEW CONSOLE                    ▁ ▢ ×    │ 260
│  > npm run build                                                    12.4 s   │
├──────────────────────────────────────────────────────────────────────────────┤
│  ● Model ready · 24 GB VRAM · 32K context          127.0.0.1:4321  ⚡ 60 fps │ 28
└──────────────────────────────────────────────────────────────────────────────┘
```

- **Left sidebar:** 280 px expanded, 64 px collapsed, resizable 240–360 px.
- **Center:** fluid, conversation max width 840 px, composer max width 840 px.
- **Right panel:** 360 px, resizable 320–440 px, collapsible to a 40 px rail or fully hidden.
- **Bottom panel:** 260 px default, resizable 160–480 px, collapsible; tabs for Terminal, Output, Problems, Preview Console.
- **Status bar:** 28 px, always visible unless full-screen preview.
- Dividers: 1 px `border/subtle` with a 6 px invisible hit area; cursor changes; double-click resets size.

### 4.3 Responsive window behaviour

| Width | Behaviour |
|-------|-----------|
| ≥ 1600 | Three columns, bottom panel docked |
| 1280–1599 | Sidebar 260 px, right panel 340 px, bottom docked |
| 1100–1279 | Sidebar auto-collapsed to 64 px; right panel becomes an overlay drawer |
| 960–1099 | Compact mode: sidebar hidden behind a rail, right panel and terminal open as overlays, composer controls collapse into an overflow menu |
| < 960 | Window cannot resize below 960×640; at 200% DPI the compact layout is used |

At 200% scaling, no control may clip, no horizontal scroll may appear in the composer, and the terminal must remain usable with horizontal scroll only inside the terminal viewport.

---

## 5. The ten interface areas

### 5.1 Application title bar and branding

**Purpose:** identity, project context, global access, window control.  
**Contents:** mark, wordmark, project breadcrumb with unsaved indicator, command palette trigger, local/offline badge, update badge, window controls.  
**States:** active/inactive window (reduce text contrast 10%, keep controls visible), maximized/restored, offline, unread update.  
**Rules:** never put more than one row of controls; no search field in the title bar (the palette handles search); drag region excludes buttons.

### 5.2 Left sidebar

**Purpose:** start work, return to work, navigate.  
**Structure:**

1. **New chat** primary button (36 px, full width, accent-tinted glass).
2. **Search** field/trigger (`Ctrl K`).
3. **Projects** list with status dot: idle, agent running, needs attention. Max 5 pinned, "All projects" link.
4. **History** grouped by Today, Yesterday, Previous 7 days, Older; virtualized; title + relative time; hover actions.
5. **Bottom:** Models, Settings, diagnostics indicator (only when action needed).

**Interaction:** click selects; `Ctrl+click` opens in a new window (post-1.0); context menu for rename, archive, delete, copy link; inline rename with Enter/Escape; keyboard navigation with arrows and Home/End.  
**Collapsed state:** 64 px rail with icon buttons, tooltips on hover and focus, active project dot.  
**Empty state:** "No conversations yet. Describe a project to begin." with a subtle sample brief.

### 5.3 Central conversation workspace

**Purpose:** the primary thinking and steering surface.  
**Layout:** scrollable message list, top fade scrim, bottom composer dock. Conversation header appears only when scrolled: project name, model, context usage.  
**Message rhythm:** 24 px between messages, 12 px within grouped tool cards, 32 px before a new task section.  
**Empty state:** Nexora mark at 32 px, one-line value statement, four starter prompts (site, app, 3D scene, fix a build), project context selector. No marketing carousel.  
**Scroll behaviour:** stick to bottom while streaming only if the user is within 80 px of the bottom; otherwise show a "Jump to latest" pill with new-token count. Never yank the viewport.  
**Task sections:** a plan card appears at the start of agent work; steps collapse into a summary when complete; evidence remains expandable.  
**Keyboard:** `Ctrl F` searches the conversation; `Esc` closes overlays before stopping generation; `Ctrl L` focuses the composer.

### 5.4 Message composer

The composer is the signature object of Nexora and receives the most design attention. See Section 6 for the full specification.

### 5.5 Model and generation status controls

- **Model pill:** 28 px, icon + short name (`Qwen3-Coder 30B`), quant badge (`Q4`), status dot (loading, ready, degraded, error). Click opens the model popover.
- **Popover:** installed models with fit badge ("Best fit", "Tight", "CPU"), context size, estimated speed range, "Manage models" link. Switching mid-conversation is allowed between turns; a separator notes the model change.
- **Generation status:** in the task panel header and composer footer: "Ready", "Planning", "Reading 3 files", "Editing", "Running build", "Repairing 1/3", "Stopped", "Blocked". No fake percentage for model thinking; use elapsed time and concrete step counts.
- **Context meter:** thin 3 px ring around the model icon or a small bar in the composer footer; tooltip shows tokens used / limit, retrieved files, and compaction state.
- **Degraded state:** "Running on CPU — slower" or "Reduced context (16K) to fit VRAM" with a details link.

### 5.6 Project explorer and file workspace

**Explorer:** tree rows 28 px, 8 px indent per level, file-type icon, name, git status letter, changed-by-agent dot. Lazy loading; `Enter` opens, `Space` previews, `F2` renames, `Delete` moves to Recycle Bin after confirmation. Filter box with fuzzy match. Ignored files are hidden by default with a toggle.  
**Tabs:** 34 px, close on middle click, overflow menu, dirty dot, drag to reorder.  
**Editor:** syntax highlighting, line numbers, minimap off by default, bracket matching, search/replace, go-to-line, read-only for files > 2 MB, binary guard. Agent edits appear as an inline diff overlay until accepted.  
**Diff view:** split or unified toggle; 13 px mono; 2 px change bars; word-level highlights; file header with path, status, +/−; accept/reject per file and per hunk; keyboard `Alt ↑/↓` to navigate hunks.  
**Empty state:** "Open a file from the explorer" with recent files.

### 5.7 Code-generation progress and task status

**Task panel header:** task title, state pill, elapsed time, stop button, overflow (pause, open log, copy report).  
**Plan list:** 28 px rows with status icon (pending circle, active pulse ring, complete check, failed x, skipped slash, blocked lock), step label, duration. Expand for details and evidence.  
**Budget meter:** compact "Calls 34/200 · Repairs 1/3 · 18m" with tooltip explanation.  
**Changes tab:** file list with +/−, status; "Review all" opens a multi-file diff; "Accept all"/"Reject all" when changes are pending.  
**Checks tab:** verification ladder (Static, Build, Tests, Runtime, Visual/A11y) with status and evidence links; stale results dimmed with "re-run".  
**Context tab:** token budget bar, retrieved files, compaction events, model and context settings.  
**Activity log:** chronological, filterable, exportable; raw tool calls collapsed by default.

### 5.8 Integrated terminal and output panel

**Tabs:** Terminal (interactive user shell), Agent Output (read-only structured commands), Problems (diagnostics), Preview Console (browser console).  
**Terminal:** xterm.js, 12.5 px JetBrains Mono, 12 px padding, custom ANSI palette aligned to the design tokens, scrollback 10,000 lines, copy/paste with confirmation for multi-line paste, clear, search, kill.  
**Agent command card:** command, cwd, exit code, duration, output (collapsible, truncated with "open full log"). Commands with non-zero exit are danger-tinted, never hidden.  
**Safety banner:** when the agent runs a command, a subtle 24 px strip says "Agent command · read-only · npm run build" and provides Stop.  
**Interactive shell:** user-initiated only; a notice explains it runs with the user's normal permissions and is not sandboxed the same way. The agent never types into the user's interactive shell.  
**Problems:** grouped by file, severity icon, message, quick open; counts in the tab label.  
**Panel states:** collapsed, docked, maximized (overlay 70% height), full-screen (post-1.0).

### 5.9 Preview and testing workspace

**Toolbar:** back/forward, reload, local URL (read-only, copyable), viewport presets (Desktop 1440, Laptop 1280, Tablet 768, Mobile 390, custom), zoom 50–150%, screenshot, open in system browser, console toggle, stop server.  
**Canvas:** neutral `#080A0D` backdrop with a 1 px device outline and 8 px radius; no skeuomorphic laptop frames. The page renders in a sandboxed iframe at the selected width; height fills the panel.  
**States:** starting (progress with server command), ready (URL + green dot), error (message, log excerpt, retry), stopped, port conflict (suggested actions).  
**Console drawer:** 200 px, level filters (error, warn, info, log), timestamp, source link, clear. Errors are summarized above the drawer.  
**Screenshots:** stored in the task report with viewport label and timestamp; compare mode highlights differences; dynamic content is flagged as non-deterministic.  
**Security:** preview is always loopback-only, sandboxed, and cannot call Nexora IPC. External links open the system browser after confirmation.

### 5.10 Settings, model management, and permissions

**Layout:** left settings nav (200 px), content max 720 px, sticky section header, no nested tabs beyond two levels.  
**Sections:** General, Appearance, Models, Inference, Permissions, Privacy & Network, Advanced, About.  
**Model manager:** cards with name, family, size, quant, context, licence link, hardware fit, status (not installed, downloading, verifying, installed, update available), actions. Download shows bytes, percentage, speed, pause/cancel; verification shows a distinct stage. A details drawer shows capabilities, known limitations, and benchmark notes.  
**Inference:** context size (presets + custom), GPU layers (auto/manual), KV cache type, threads, backend (Auto/CUDA/Vulkan/CPU), idle unload, "Restore recommended". Every control shows the effect and a warning when outside recommended range.  
**Permissions:** matrix by project: Read, Write, Run commands, Network, Secrets, Outside workspace, Git write. Each row shows current grant, scope, last used, and Revoke. A global "Reset all permissions" action.  
**Privacy & Network:** telemetry (off, locked at 1.0 unless opted in), crash reports, update checks, model download host, network activity log, export/delete data.  
**About:** version, build hash, runtime versions, licence acknowledgements, open-source notices, diagnostics export, "Run setup again".  
**Destructive actions:** danger zone with typed confirmation for deleting all data; per-item delete uses a normal confirmation with scope explanation.

---

## 6. Composer specification (major element)

### 6.1 Anatomy

```
┌──────────────────────────────────────────────────────────────────────┐
│  [context chips: project · file · selection]                         │  optional row
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  Describe what you want to build…                              │  │  textarea
│  │                                                                │  │
│  └────────────────────────────────────────────────────────────────┘  │
│  [＋] [project ▾] [attach]                          [model ▾] [◔] [➤] │  toolbar
└──────────────────────────────────────────────────────────────────────┘
```

- **Container:** max width 840 px, centered, 24 px radius, `glass/panel`, 1 px border, inner top highlight, `shadow/high` on focus only.
- **Padding:** 14 px top, 16 px horizontal, 10 px bottom. The textarea has no visible border.
- **Textarea:** 15/24 Inter, off-white, placeholder `text/tertiary`; auto-grow from 1 to 6 lines (64–200 px), then internal scroll. No resize handle.
- **Footer toolbar:** 32 px controls, 8 px gaps; left controls (attach, project context, slash commands), right controls (model, context meter, send/stop).
- **Send button:** 32 px circle, accent fill when enabled, `text/inverse` arrow; disabled = `bg/raised` + `text/tertiary`. While generating, morphs to a stop square with a 160 ms crossfade; hover shows "Stop (Esc)".
- **Focus:** 1 px `accent/base` border plus a 3 px `accent/subtle` outer ring, 160 ms; no layout shift.
- **Drag-over:** 1 px dashed accent border, 2% accent tint, label "Drop files to attach".

### 6.2 States

| State | Visual | Copy / behaviour |
|-------|--------|-----------------|
| Default | Glass surface, subtle border, placeholder | "Describe what you want to build…" |
| Hover | Border 8% brighter | Cursor text |
| Focus | Accent border + ring | Placeholder remains, caret visible |
| Typing | Send enabled, character/token meter updates | Enter sends; Shift+Enter newline |
| Dragging | Dashed accent overlay | "Drop files to attach" |
| Generating | Send → Stop; footer shows step + elapsed; textarea remains editable for queued input | "Stop" tooltip; queued message allowed only after current turn |
| Stopping | Stop shows a small progress ring | "Stopping…" until runtime acknowledges; then partial message marked Stopped |
| Stopped | Neutral footer chip "Stopped" | Partial content retained and labelled |
| Error | Danger border, inline error row above toolbar | "Generation failed. Retry" + details |
| Disabled | 60% opacity, no focus | Reason shown: "Load a model to start" |
| Offline | Amber cloud-off chip | "Offline — local model ready" or "Model missing" |
| Model loading | Small ring in model pill; send disabled | "Loading model… 42%" |
| Permission needed | Amber lock chip above input | "Waiting for permission: run npm install" with Review |
| Read-only | No send button; lock icon | "This conversation is archived" |
| Context full | Meter danger; warning row | "Context 96% — Nexora will compact before sending" |

### 6.3 Controls and menus

- **Attach:** opens file picker; accepts text/code and images (images only if the active model supports vision). Chips show icon, name, size, remove. Limits: 10 files, 1 MB each for text, 10 MB each for images; reject binaries with a clear message.
- **Project context:** toggle "Use project context" with project name; opens a popover to include/exclude folders, pin files, and see indexed file count. Default on for project conversations.
- **Slash commands:** `/plan`, `/fix`, `/test`, `/preview`, `/explain`, `/checkpoint`, `/stop` — discoverable via `/` menu, keyboard navigable, never sent as raw text unless unknown.
- **Model selector:** see 5.5. Shows only installed models; "Download a model" opens the manager.
- **Context meter:** thin ring; hover/focus tooltip with tokens, retrieved files, and compaction. Turns amber at 80%, red at 95%.
- **Send/stop:** primary; `Enter` send, `Shift+Enter` newline, `Ctrl+Enter` send, `Esc` stop (when generating), `Ctrl+L` focus composer.

### 6.4 Input behaviour and edge cases

- **IME:** never send while `compositionstart` is active; Enter commits the composition first. Test with Japanese and Vietnamese IMEs.
- **Paste:** plain text by default; pasted code > 20 lines becomes a collapsible attachment chip "Pasted code · 84 lines" to keep the composer readable.
- **Long input:** > 4,000 characters shows a soft counter; > 32,000 characters warns and suggests attaching a file.
- **Undo:** native textarea undo within the composer; clearing after send is not undoable, but the draft is stored until the next send.
- **Draft persistence:** per conversation, saved locally, restored after restart; cleared on send.
- **Queued input:** while generating, the user may type; the send action queues one message and shows "Queued". No multiple queue at 1.0.
- **Window resize:** footer controls collapse into an overflow menu below 720 px content width; the textarea never shrinks below 48 px.

### 6.5 Accessibility

- Textarea has an accessible label and description; placeholder is not the only label.
- Status changes are announced via a polite live region, throttled to at most one announcement every 2 s; streaming tokens are never announced.
- Send/stop button has a stable accessible name that changes with state; shortcut is exposed via `aria-keyshortcuts`.
- All chips and controls are reachable in DOM order: context → textarea → attach → project → model → send.
- Focus returns to the textarea after send, after stopping, and after closing a popover.
- Reduced motion removes the send/stop morph and caret pulse.

### 6.6 Composer performance

- The composer is isolated from the message list render tree; streaming must not re-render the textarea or its toolbar.
- Token count updates are debounced at 120 ms and computed off the UI thread where possible.
- Auto-grow uses `requestAnimationFrame` and a cached line height; no layout thrash per keystroke.
- Glass blur is one layer; the composer never nests another blurred element inside it.

---

## 7. Message and streaming design

### 7.1 Message types

| Type | Treatment |
|------|-----------|
| User | Right-aligned raised bubble, max 75% width, 14/22, 16 px padding, 16 px radius with a 4 px corner toward the avatar |
| Assistant | Full-width, no bubble, 36 px mark column, 15/26, max 760 px measure |
| System | Centered or inline muted card, 13/20, no avatar, e.g. model switched, context compacted |
| Tool call | Compact 32 px row inside assistant flow: icon, verb, target, status, duration; expandable output |
| Plan | Bordered card with step list; collapses to "Plan · 6 steps · completed" after the task |
| Diff | File header + unified diff preview (max 12 lines) + "Open diff" |
| Verification | Check ladder card with status chips and evidence links |
| Error | Inline danger card with message, retry, details, diagnostics |
| Attachment | File card with name, size, type, remove before send |

### 7.2 Streaming behaviour

- Text appears as it arrives; a 2 px accent caret follows the last character. No per-character fade or typewriter effect.
- The composer remains interactive; the message list scrolls only when pinned to bottom.
- Tool calls render as soon as the structured call is validated, not after the tool completes; status transitions pending → running → success/failure.
- A generation footer shows current step and elapsed time, then collapses to a summary when complete.
- Partial output on stop is preserved with a "Stopped" chip and a note that the response is incomplete.
- If the runtime disconnects, the message shows "Connection to model lost" with Retry and a link to diagnostics; no silent retry that duplicates text.

### 7.3 Rich content safety

- Markdown is rendered with raw HTML disabled; links are sanitized; external links open the system browser.
- Code blocks are highlighted with a local highlighter (Shiki or equivalent) with a custom theme; copy button; language label; wrap toggle.
- File links resolve only inside the project; path traversal and absolute paths are rejected.
- Images from project files are served through a constrained local protocol; remote images are not auto-loaded in messages.
- Mermaid or diagram rendering, if added, must be sandboxed; not required at 1.0.

---

## 8. Status, evidence, and honesty in the interface

### 8.1 Verification chips

| Chip | Meaning | Colour | Icon |
|------|---------|--------|------|
| Generated | Files were written; no checks run | neutral | file-plus |
| Built | Build command exited 0 | info | hammer |
| Tests passed | Test command exited 0 and produced a result | success | check |
| Preview loads | Local server returned a valid response and no fatal console error | success | monitor |
| Visually checked | Screenshots captured and compared/reviewed | success | image |
| Unverified | Could not check, check skipped, or evidence stale | warning | alert |
| Failed | A check ran and failed | danger | x |
| Blocked | Needs permission, input, or a missing dependency | warning | lock |

Rules:

- A chip is only shown when its evidence exists and is tied to the current file hashes.
- After any edit, prior check chips become "Stale" until re-run.
- The final report uses the same vocabulary as the chips; no free-text upgrade from "built" to "works".
- The UI never renders a success colour for a check that did not run.

### 8.2 Task report card

At the end of a run, the conversation shows a compact report:

```
Task complete — 3 of 4 checks passed
Changed 6 files · +412 −87
✓ Build            npm run build            12.4 s
✓ Tests            npm test                 8.1 s
✓ Preview          http://127.0.0.1:4321    200 OK
! Visual           screenshots unavailable  unverified
Open diff · View report · Restore checkpoint
```

If the task failed:

```
Task stopped — build still failing (3 attempts)
Last error: TS2307 Cannot find module '@/components/Hero'
Files changed: 2 · Checkpoint created
What I tried · Open log · Restore checkpoint · Ask for help
```

The report never says "done" when a required check failed.

---

## 9. Empty, loading, error, and offline states

| Context | Empty | Loading | Error | Offline |
|---------|-------|---------|-------|---------|
| Conversation | Mark + 4 starter prompts | Plan skeleton with shimmer on 2 px lines only | Inline card + retry | "Local model ready" or "No model available offline" |
| Explorer | "Open a folder" + recent | Skeleton rows (max 8) | "Could not read folder" + path + retry | Normal; local files unaffected |
| Editor | Recent files | File skeleton | "File is binary/large" | Normal |
| Terminal | Prompt | Starting shell indicator | "Shell failed to start" + details | Normal |
| Preview | "Start preview" | Server starting + command | Error + log + retry | "Preview needs local server; no network required" |
| Models | "No models installed" + recommended | Download progress with stages | "Download failed" + resume | "Connect to download; existing models work offline" |
| Search | Recent searches | Inline spinner ≤ 300 ms | "Index unavailable" + rebuild | Local index works offline |

**Loading rules:** skeletons for structure, progress for work with known length, spinners only for unknown short waits, and always a label after 1 s. No full-screen spinner blocks the shell.

---

## 10. Onboarding flow

**Step 1 — Welcome.** Nexora mark, one-sentence positioning, three bullets: local inference, project control, verifiable results. Buttons: "Set up Nexora" / "Explore without a model".

**Step 2 — Privacy and network.** Plain-language disclosure: what stays local, what needs network (model download, dependency install, update check), no telemetry by default. Link to full privacy page. Continue.

**Step 3 — Hardware check.** Animated but restrained scan (max 1.2 s, skippable), then a card: CPU, RAM, GPU, VRAM, disk, WebView2. Recommendation: "Best fit: Qwen3-Coder 30B Q4 · 19 GB · tight on 12 GB VRAM" with an explanation and a smaller alternative. No performance promises.

**Step 4 — Model download.** Licence link and acceptance, size, disk check, download button. Progress with stages (download → verify → install → warm-up). Pause/cancel. "Skip for now" always available.

**Step 5 — First project.** Three cards: "Create a website", "Open a folder", "Start empty". Optional sample brief. Finish.

**Rules:** no account, no email, no mandatory survey. Onboarding can be completed offline if a model is already installed (e.g., restored app data). Every step is keyboard navigable and screen-reader labelled.

---

## 11. Accessibility specification

- **Standard:** WCAG 2.2 AA for the application UI. Generated websites are checked against the same baseline where practical.
- **Contrast:** 4.5:1 body text, 3:1 large text and UI boundaries, 3:1 focus indicators. Glass surfaces are tested against worst-case backgrounds.
- **Focus:** 2 px `accent/base` ring with 1 px offset; never removed; visible on dark and light surfaces; logical DOM order; focus trapped in dialogs; Escape closes; focus returns to the trigger.
- **Keyboard map (core):**

| Shortcut | Action |
|----------|--------|
| `Ctrl K` | Command palette |
| `Ctrl L` | Focus composer |
| `Enter` / `Shift+Enter` | Send / newline |
| `Ctrl Enter` | Send |
| `Esc` | Stop generation or close overlay |
| `Ctrl Shift N` | New chat |
| `Ctrl O` | Open project |
| `Ctrl P` | Quick open file |
| `Ctrl Shift F` | Search project |
| `Ctrl B` | Toggle sidebar |
| `Ctrl J` | Toggle bottom panel |
| `Ctrl Shift P` | Toggle preview |
| `Ctrl ,` | Settings |
| `Alt ↑/↓` | Previous/next diff hunk |
| `F6` | Cycle regions (sidebar, conversation, context, terminal) |

- **Screen readers:** landmarks for title bar, navigation, main, complementary, terminal; live regions only for status changes; tooltips available on focus; icons have accessible names or are hidden.
- **Reduced motion:** honoured at the OS level and via an in-app override; no looping ambient animation.
- **Reduced transparency / high contrast:** glass falls back to opaque surfaces; Windows forced-colours mode maps tokens to system colours; borders remain visible.
- **Text scaling:** 200% without loss of function; no fixed-height text containers that clip.
- **Motor:** no drag-only actions; all drag interactions have keyboard/menu equivalents; 32 px minimum targets.
- **Cognitive:** plain language; no time-limited dialogs; confirmations state consequences; destructive buttons are never the default focus.

---

## 12. Microcopy guidelines

| Situation | Use | Avoid |
|-----------|-----|-------|
| Empty conversation | "Describe what you want to build." | "Let's create something amazing!" |
| Model loading | "Loading Qwen3-Coder 30B — 42%" | "Warming up the magic…" |
| Build running | "Running npm run build" | "Working hard…" |
| Build failure | "Build failed: 2 TypeScript errors in Hero.tsx" | "Oops! Something went wrong." |
| Unverified | "Unverified — no browser available for screenshots" | "Probably fine!" |
| Permission | "Nexora wants to run `npm install`. This uses network access." | "Allow access?" (without scope) |
| Stop | "Stopped. Partial response kept." | "Cancelled." (without consequence) |
| Completion | "3 checks passed · 1 unverified" | "Perfect! Your site is ready." |
| Offline | "Offline. Local model and files still work." | "No internet connection." (without impact) |
| Destructive | "Delete 14 conversations? This cannot be undone." | "Are you sure?" |

---

## 13. Performance and rendering constraints for the UI

- Virtualize message history above 200 messages and file trees above 500 visible rows.
- Keep the streaming message in its own render boundary; never re-render the shell or composer per token.
- Batch streamed deltas at 20–30 Hz; use `requestAnimationFrame` flushes.
- Use CSS containment (`contain: layout paint`) for message items and panels where safe.
- Avoid large box shadows on scrolling content; use borders and background differentiation.
- Blur budget: at most three surfaces; measure GPU time on Intel Iris Xe class hardware.
- Images in messages are lazy-loaded, size-capped, and decoded off the main thread where possible.
- Syntax highlighting is incremental; large code blocks (> 2,000 lines) render as plain text with a "highlight" action.
- No layout shift when tool cards, chips, or diffs appear; reserve space or animate height only with a measured container (post-1.0 if costly).

---

## 14. Design deliverables for Phase 2 and Phase 3

Phase 2 must produce, before UI implementation:

1. **Token file** (JSON/CSS variables) covering colour, type, spacing, radius, elevation, motion, and glass, with light-theme-ready naming.
2. **Component inventory** with states: button, icon button, input, textarea, select, popover, dialog, tooltip, tabs, tree row, message, tool card, diff card, status chip, progress, toast, empty state, permission card.
3. **High-fidelity screens** at 1440×900 and 960×640 (compact) for: onboarding, empty conversation, active generation, task report with failure, project explorer + editor + diff, preview, terminal, model manager, permissions, settings, command palette.
4. **Composer prototype** with all states, IME test notes, keyboard behaviour, and reduced-motion variant.
5. **Icon vector master** at the specified geometry, exported to ICO/PNG and tested at 16 px.
6. **Motion spec** with durations, easing, and reduced-motion mapping.
7. **Usability test** with 5 target users covering first project, stop/cancel, permission prompt, and finding evidence; report comprehension of "verified vs unverified".

No design deliverable is final until the owner approves the icon and the visual direction.

---

## 15. Anti-patterns (explicitly rejected)

- Purple-to-pink gradients, neon glows, glass everywhere, blurred body text.
- Chat bubbles for every message, giant avatars, emoji reactions.
- Fake "thinking" percentages, fake agent progress, fake terminal chatter.
- Hiding failures behind "completed with warnings" without a visible warning chip.
- Auto-scrolling that fights the reader.
- Icon-only toolbars without tooltips or accessible names.
- Dense IDE chrome that makes the conversation secondary.
- Modal dialogs for routine actions.
- Motion that delays work or cannot be disabled.
- Copying another product's layout, iconography, or brand assets.
- Claiming "offline" while silently contacting a server.
- Using colour alone to communicate status.
- Showing a success state for a check that was skipped.

---

## 16. Design acceptance checklist

- [ ] Every primary flow works at 960×640 and 1440×900, 100% and 200% scaling.
- [ ] Composer supports every state in Section 6.2 and passes IME tests.
- [ ] Glass layers never exceed three and never reduce body-text contrast below 4.5:1.
- [ ] Reduced motion and reduced transparency are honoured.
- [ ] All core actions are keyboard reachable with visible focus.
- [ ] Verification chips match evidence and turn stale after edits.
- [ ] Icon is recognizable at 16 px in monochrome and colour.
- [ ] No screen claims success without supporting checks.
- [ ] No remote fonts, images, or scripts are required for the app shell.
- [ ] Design tokens are implemented as variables, not scattered literals.
