# Developer portal captures

Genuine headless Chromium 153 captures taken on 2026-10-27 against the **dev
server** (`vite` on :5173) with the portal server running and its `/api` proxy
active — that is, the real cookies, the real CSRF header, and the real session
behaviour, not a mocked state.

Method for every image: network idle → `document.fonts.ready` → 400 ms → two
animation frames → capture. Each page reported zero page errors; the only
console entries were the expected `401` from the signed-out session probe
(`GET /api/portal/session`), which is how the UI learns nobody is signed in.

| File | What it shows |
| --- | --- |
| `01-signin-desktop.png` | 1440 × 1000. `/admin` signed out: centred panel, email + password fields, show/hide control, primary sign-in button, link back to the public site |
| `02-overview-desktop.png` | 1440 × 1000. Dashboard shell (bar, account identity + sign out, section rail) and the overview facts |
| `03-website-desktop.png` | 1440 × 1000. Website status: build, indexing, deployment (unverified), installer (none), public route inventory |
| `04-content-desktop.png` | 1440 × 1000. Content: explicit "not configured" state, no editing controls |
| `05-support-desktop.png` | 1440 × 1000. Support: explicit "not configured" state, links to the public FAQ and Support pages |
| `06-settings-desktop.png` | 1440 × 1000. Account, session, the real sign-out button, password-rotation command, security guidance, configuration status |
| `07-signin-mobile.png` | 390 × 844 touch context at `deviceScaleFactor: 2` (PNG is 780 × 1688) |
| `08-overview-mobile.png` | the same context, signed in: the section rail becomes a two-row grid above the content |
| `09-website-mobile.png` | the same context on the website section, showing the route inventory stacked below 430 px |

The account visible in these images is a **local development account**
(`developer@nexora.invalid`, an address in the reserved `.invalid` TLD) created
with `server/provision-owner.mjs` against a git-ignored `.portal-data`
directory. It is not a production identity and no credential is recorded here.
