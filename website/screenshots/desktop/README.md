# Desktop screenshot baseline — 1440 × 1000

> **Superseded — do not treat these images as the current design.**
> These captures record the site as it was on 2026-10-26, before the pre-Phase 11
> UI refinement. That refinement removed the lower "meet Nexora" home section and
> the right-hand hero illustration, centred the primary navigation, removed the
> duplicated footer navigation, and reworked the type scale and spacing across
> every page. The PNGs below are therefore a **historical** baseline: the header,
> the home page, the footer, and the type sizes in these images no longer match
> the code in `website/src`. Re-capture with the same method when a current
> baseline is needed; do not edit these images.

**Exceptions — `11-home-particle-field.png`, `12-cursor-home.png`,
`12-mobile-menu.png` and the three `13-*.png` captures.** These are *not*
superseded; they were captured after the UI refinement, Phases 11–13, and they
show the current design.

- `11-home-particle-field.png` — 1440 × 1000 viewport capture, pointer resting in
  the open area right of the hero.
- `12-cursor-home.png` — 1440 × 1000 viewport capture with the Phase 12 custom
  cursor visible over the hero.
- `12-mobile-menu.png` — 390 × 844 capture with the mobile menu open, taken in a
  touch context (no emulated fine pointer), so no custom cursor is present.
  **Its header is superseded by `13-mobile-menu.png`**: the page behind the menu
  is unchanged, but the flat header bar in this image was replaced by the Phase 13
  glass capsule. It is kept because it records the Phase 12 menu behaviour.
- `13-header-desktop.png` — 1440 × 1000 **viewport** capture of the home page
  showing the Phase 13 glass navigation capsule (content-sized, centred, 808 × 66
  px, `blur(18px) saturate(1.5)` behind a 78 % white tint).
- `13-mobile-home.png` — 390 × 844 viewport capture in a touch context
  (`deviceScaleFactor: 2`, so the PNG is 780 × 1688): capsule with the wordmark
  and the 44 × 44 toggle, and the mobile hero type.
- `13-mobile-menu.png` — the same page and context with the menu open: eight
  44 px rows in a panel aligned to the capsule's edges, and the toggle drawn as a
  close cross.

All three were captured with the same settle-then-capture procedure as the rest
of this folder (network idle, `document.fonts.ready`, 400 ms, then two animation
frames), each page reporting zero console and zero page errors.

**Also superseded — the paragraph below about the original exception.** This one image is *not* superseded. It
was captured after both the UI refinement and Phase 11, and it is the only image
here that shows the current design. It is a **viewport** capture (1440 × 1000,
not full-page) taken with the pointer resting in the open area to the right of
the hero so that the cursor influence is visible without covering the copy. The
ten images above it remain the historical 2026-10-26 baseline.

Full-page screenshots of every public page of the Nexora website, captured with a real
headless Chromium browser driving the site's own development server.

These images are a **visual baseline**. They record what the pages looked like at one
viewport on one date. They are **not** proof of functional correctness, and they say
nothing about keyboard operation, screen-reader behaviour, mobile layouts, or any
viewport other than the one documented below.

## Capture record

| Field | Value |
| --- | --- |
| Captured | 2026-10-26, 20:05:51 → 20:06:05 UTC (2026-10-27, 01:35:51 → 01:36:05 IST, UTC+05:30) |
| Browser | Chromium `153.0.8010.0` (`HeadlessChrome/153.0.8010.0`), Linux x86_64 |
| Browser engine | Headless Chromium built for serverless use, distributed by the `@sparticuz/chromium` npm package (v153.0.0) |
| Automation | `puppeteer-core` 25.16.0 driving the browser over the DevTools Protocol |
| Viewport | **1440 × 1000 CSS px**, `deviceScaleFactor: 1`, desktop context (no mobile emulation, no touch emulation) |
| Page zoom | 100 % (browser default; no `zoom`, no `deviceScaleFactor` scaling, no CSS `transform: scale`) |
| Capture mode | Full-page vertical capture (`fullPage: true`, single pass), PNG, sRGB |
| Target | The site served by its Vite **development** server at `http://127.0.0.1:5173` — not the production build (see Limitations) |
| Output | 10 PNGs, 1440 px wide, original full-page height, no downscaling, no cropping, no compositing |

Before each capture the browser was given time to settle: `load` event, then network idle
(600 ms with no requests), then `document.fonts.ready`, then a further 400 ms. Each page
reported zero console errors and zero page errors during capture.

## Genuine captures, not reconstructions

Every PNG in this folder was produced by a real browser rendering the live site over HTTP.
No screenshot was produced from server-side rendered markup, a static export, a design
mock-up, an AI-generated image, or any kind of reconstruction. Nothing was retouched,
annotated, upscaled, or stitched together, and no page was modified in any way to improve
how it looks.

The browser was genuinely present: it is Chromium build 153.0.8010.0, and the font it
actually used for body text was read back from the rendering engine
(`CSS.getPlatformFontsForNode`), which reported **Open Sans** — see the font note below.

## Route → screenshot mapping

All ten routes were attempted; all ten succeeded. "Height" is the captured full-page height
in CSS px. The H1 column is the heading the browser actually rendered, compared against the
heading the page is supposed to show.

| # | File | Route | HTTP | Full-page height | Rendered H1 | Result |
| --- | --- | --- | --- | --- | --- | --- |
| 01 | `01-home.png` | `/` | 200 | 1639 px | “Your AI coding workspace. On your terms.” | Success |
| 02 | `02-features.png` | `/features` | 200 | 4060 px | “Built around a more local coding workflow.” | Success |
| 03 | `03-how-it-works.png` | `/how-it-works` | 200 | 3362 px | “Understand the workflow, before you begin.” | Success |
| 04 | `04-download.png` | `/download` | 200 | 2915 px | “Nexora for Windows.” | Success |
| 05 | `05-privacy.png` | `/privacy` | 200 | 7681 px | “Where your code and conversations go.” | Success |
| 06 | `06-security.png` | `/security` | 200 | 6094 px | “What Nexora does to protect your machine.” | Success |
| 07 | `07-about.png` | `/about` | 200 | 4117 px | “About the Nexora project.” | Success |
| 08 | `08-faq.png` | `/faq` | 200 | 3118 px | “Frequently asked questions.” | Success |
| 09 | `09-support.png` | `/support` | 200 | 4322 px | “Getting help with Nexora.” | Success |
| 10 | `10-not-found.png` | `/this-route-does-not-exist` | 200 | 1000 px | “This page doesn’t exist.” | Success (see note) |

Notes on the mapping:

- The route list comes from the site's own router (`src/config/routes.ts` and
  `src/App.tsx`). There are nine declared public routes plus a catch-all `*` that renders
  the not-found page; the tenth entry above is a deliberately invalid path used to capture
  that catch-all. No other public route exists.
- **HTTP 200 for the not-found page is expected, not an error.** This is a client-side
  single-page app served by a static dev server: the server returns the same HTML shell for
  any path and the router renders the not-found view in the browser. The captured page is
  the real not-found view — it shows “ERROR 404”, the not-found heading, and the “Back to
  home” button.
- Each page was confirmed to be the intended page rather than a blank tree, an error, or an
  unexpected fallback: exactly one `<h1>` per page, matching text, non-trivial rendered body
  text, and the expected header link marked `aria-current="page"` on the nine content pages.
- No route failed, so there are no failed captures to report. Nothing was omitted.

## Visual review

Reviewed at full resolution where the region mattered, plus whole-page review of all ten
images. The following was checked on every route: header and footer overlap, broken or
missing navigation, horizontal overflow, content width and desktop spacing, truncated text,
unintended empty sections, render failures, and the links present in the rendered DOM.

**One defect found, reported not fixed:**

1. **Primary navigation items are not vertically aligned — `01-home.png` … `10-not-found.png`
   (all ten, in the header).** The first item, “Features”, sits **4 px higher** than the
   other seven items on every route, whichever page is active. Cause, traced in the rendered
   styles: `styles/base.css` applies `li + li { margin-top: var(--space-2) }` (8 px) as a
   prose-list rule. The footer list and the mobile menu list both reset it
   (`SiteFooter.css:72`, `SiteHeader.css:137`), but the desktop navigation list
   (`.site-header__nav-list`) does not. Because that list is a flex row with centred items in
   a fixed-height box, the extra 8 px top margin on items 2–8 pushes them 8 px down inside
   their own boxes, which re-centres them 4 px lower than the first item. It is most visible
   at 100 % zoom on the header. **Not fixed, per the terms of this audit** — it is recorded
   here as evidence for the next phase to address.

**Checks that passed**, so they are recorded as absent rather than silently skipped:

- No horizontal overflow on any route: `document.documentElement.scrollWidth` equals 1440 on
  all ten pages, and no element inside `main`, `header`, or `footer` extends past the
  viewport edge on either side.
- No clipped text: no visible element on any route has hidden vertical overflow with
  content taller than its box, and no text element is truncated horizontally. The only
  elements with hidden overflow are `.visually-hidden` screen-reader spans, which is their
  purpose.
- The footer sits flush with the end of every page (no trailing gap, no overlap with page
  content), and the sticky header does not cover page content: with the header pinned at
  65 px, in-page anchor targets clear it (`html` carries `scroll-padding-top`), measured on
  `/faq#what-is-nexora` as target top 80 px against header bottom 65 px.
- No broken links in the rendered DOM: every internal `href` points at one of the nine
  declared routes, every `#fragment` resolves to an element that exists, and there are no
  empty, placeholder, or `javascript:` hrefs.
- No unintended empty sections and no missing content blocks; every heading level is
  populated. No images are used on these pages at all (all visuals are CSS or inline SVG),
  so there is no missing alternative text to report.
- No console errors or uncaught page errors on any route.

**Not a defect, recorded so it is not mistaken for one:** on `10-not-found.png` the content
ends well above the footer, leaving a large blank area. That page fills the viewport height
so the footer stays at the bottom, which is intended layout behaviour, not a rendering
failure.

## Limitations of this capture

Read these before treating any pixel as authoritative.

- **Fonts are substituted.** The site's stack is
  `-apple-system, BlinkMacSystemFont, "Segoe UI", "Helvetica Neue", Arial, "Noto Sans",
  sans-serif`. On Linux the first two resolve to nothing, and Segoe UI, Helvetica Neue,
  Arial, and Noto Sans are all absent from the capture environment and could not be
  installed. The browser
  rendered body text in **Open Sans** (confirmed from the engine, not assumed), which was
  made available locally and mapped onto the sans-serif stack for the capture. Open Sans is
  metrically close to, but not identical with, the stack's own Noto Sans fallback. **Line
  breaks, text widths, and the number of lines in headings can therefore differ from what a
  Windows visitor sees.** Text sizes, weights, colours, and spacing are the site's own.
- **The site was captured from its development server, not the production build.** The
  screenshots show the Vite dev server at `127.0.0.1:5173`, which is what the running site
  was. The production build (`vite build`) produces the same markup and styles in minified
  form and was not captured here; if the release needs a production-build baseline, that is
  a separate capture.
- **This is headless Chromium on Linux x86_64, not a desktop browser on Windows.** Nobody
  looked at these pages in a headed window. Rendering used software rasterisation
  (`--disable-gpu`) with a forced sRGB colour profile, no font hinting
  (`--font-render-hinting=none`), and a 1× device pixel ratio — so the PNG pixels equal CSS
  pixels one-to-one, with no HiDPI scaling. On a Windows display with different ClearType
  settings and a fractional or 2× scale factor, subpixel rendering and the effective pixel
  size of hairlines and borders will differ.
- **Scrollbars were hidden for capture.** Chromium was launched with `--hide-scrollbars` so
  the full 1440 px width is content. With a classic 15 px scrollbar, the layout width would
  be about 1425 px. This does not hide any layout defect at 1440, but it does mean the
  captured width is marginally wider than a real window with visible scrollbars.
- **Full-page capture is a single tall render, not a scroll-through.** Sticky elements
  therefore appear once, at their resting position (the header at the top), rather than
  repeated down the page. Nothing was scaled or stitched.
- **Only one viewport exists in this baseline:** 1440 × 1000 CSS px, zoom 100 %, desktop
  context. No tablet, narrow-desktop, or mobile widths were captured, so these images
  support **no** statement about responsive behaviour at any other size.
- **Only the resting state is captured.** Hover, focus, open/closed disclosure states, and
  menu states are not shown. The FAQ page's questions are captured in their default
  collapsed state; the answer text is present in the page and searchable, but not visible in
  the screenshot.
- **What these images cannot show:** keyboard operability, focus order, focus visibility,
  screen-reader output, colour-contrast conformance (measured separately, not re-measured
  here), JavaScript runtime behaviour beyond first paint, or any behaviour behind a click.

## Environment used

Captured in a Linux sandbox (`x86_64`, glibc 2.36) that initially had no browser at all and
no way to install one system-wide. The route taken, recorded so the setup can be repeated or
audited:

1. `@sparticuz/chromium@153.0.0` from npm provided the Chromium binary, and its bundled
   `al2023.tar.br` payload provided the shared libraries the binary needs.
2. `puppeteer-core@25.16.0` from npm drove that binary over the DevTools Protocol.
3. Open Sans (shipped inside the same package) was placed in the user font directory and
   mapped onto the sans-serif stack through a user-level `fontconfig` file.

All of that was installed **outside** the repository. No dependency, lockfile, manifest, or
build script in the repository was added to or changed, and none of those tools are
committed. The repository's only additions for this task are the ten PNGs and this file.

`fc-list` and `ldconfig` are not installed in this environment, so library and font
availability was verified directly (by attempting to load each library and by inspecting the
installed font files) rather than by trusting those commands' empty output.
