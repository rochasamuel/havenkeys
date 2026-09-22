# Marketing / download website — design

> This software has not undergone an independent security audit.

## 1. Motivation

HavenKeys has no public presence: no way to point someone at a page that
explains what it is, and no way to hand them an installer without walking
them through building from source. This adds a small static marketing site
that explains the product honestly, links to a real download, and carries
the legal pages (Privacy Policy, Terms of Service) that both a public site
and, eventually, browser-store submissions need.

This is additive: it does not touch the desktop app, the extension, the
server, or any crate. It is a new, independent workspace member plus a new
CI workflow.

## 2. Goals

* A clean, honest marketing page at `havenkeys.net` that explains the
  product and its security model without overclaiming.
* A Download page that always points at real, current installers — never a
  stale or hand-maintained link.
* A repeatable way to produce those installers for Windows, macOS, and
  Linux from CI, published as GitHub Release assets.
* Privacy Policy and Terms of Service pages, accurate to what the product
  and the website actually do.
* Visually consistent with the desktop app and extension: same color
  tokens, same type, same honest tone — recognizably the same product.

## 3. Non-goals

* Submitting to the Chrome Web Store or Firefox Add-ons (separate, mostly
  manual step — screenshots, listing copy, review queue). This project only
  ships the legal pages a submission would need to link to.
* A CMS, blog, or any content that changes without a code deploy.
* Any account system, contact form, or server-side logic. The site is
  static; its only network calls are to the public GitHub API.
* macOS notarization/code-signing hardening beyond what `tauri-action`
  does by default — the installers are downloadable but may show OS
  "unidentified developer" warnings until signing is set up. Noted as a
  known limitation, not solved here.

## 4. Architecture

New workspace member:

```text
apps/web/
├── src/
│   ├── pages/
│   │   ├── Home.tsx
│   │   ├── Download.tsx
│   │   ├── Privacy.tsx
│   │   ├── Terms.tsx
│   │   └── Security.tsx
│   ├── components/
│   ├── lib/
│   │   └── releases.ts        # fetches the latest GitHub Release
│   ├── App.tsx
│   └── main.tsx
├── public/
├── index.html
├── vite.config.ts
├── package.json
└── tsconfig.json
```

Stack: Vite + React + TypeScript + `react-router`. Styling: plain CSS
modules or hand-written CSS importing `@havenkeys/ui`'s existing
`tokens.css` — no new design-token source of truth, no Tailwind, no
component library. This keeps the site's visual identity mechanically tied
to the desktop/extension tokens: if `packages/ui` changes a color, the site
picks it up on its next build.

Added to `pnpm-workspace.yaml` (already glob-matches `apps/*`, so no change
needed there) and to root `package.json` as a `--filter @havenkeys/web`
script if useful for local dev (`pnpm --filter @havenkeys/web dev`).

Deployment: Vercel, connected directly to the GitHub repo, root directory
`apps/web`, framework preset "Vite". Custom domain `havenkeys.net` (apex)
and `www.havenkeys.net` (redirecting to apex) — once a Vercel project
exists I'll give you the exact DNS records to add at your registrar.
`vercel.json` in `apps/web` adds the SPA rewrite (`/* → /index.html`) so
client-side routes resolve on refresh.

No backend, no database, no server code. The only outbound network calls
the deployed site makes are to `api.github.com` (public, unauthenticated,
rate-limited but fine for this traffic level) to find the latest release,
and to Vercel's own analytics endpoint (see §7).

## 5. Design system

Supabase's *layout conventions*, HavenKeys' *own palette* — nothing here
introduces new brand colors:

* Dark by default (`--bg: #0f1614`), matching the desktop app's default
  theme. No light-mode toggle needed for a marketing site.
* Single accent color: `--brass` (`#c9a45c`) for every CTA, link, and
  highlighted term — mirrors its role as `--primary-bg` in the app.
* Type: `Hanken Grotesk` for headings/body (`--font`), `JetBrains Mono`
  (`--mono`) for anything code-shaped — version numbers, the `otpauth://`
  example, permission names.
* Layout patterns borrowed from Supabase's marketing site: a left-aligned
  hero with a code/terminal-style visual instead of a stock photo, a
  bordered feature-card grid, a slim sticky top nav, generous vertical
  whitespace, no carousel/parallax gimmicks.
* Tone constraint carried over from `CLAUDE.md` §50 and the README: no
  "military-grade," "unhackable," or "100% secure" language anywhere on the
  site. The homepage repeats the same audit disclaimer already in the
  README, verbatim.

## 6. Pages

**Home** — hero (name, one-line pitch, download CTA), "how it works"
(local-first, key hierarchy in plain language, server holds ciphertext it
can't read), a feature grid (logins, secure notes, TOTP, password
generator, autofill, cross-device sync via your own server), a condensed
security section that links out to `docs/security-model.md` and
`docs/threat-model.md` on GitHub rather than restating them, footer with
links to GitHub, License, Privacy, Terms.

**Download** — one card per platform (Windows `.msi`, macOS `.dmg`, Linux
`.AppImage` and `.deb`), each button resolved at page-load time from the
latest GitHub Release (§8) so it always points at a real, current asset.
Shows the version number and a "View all releases" link to the GitHub
Releases page for older builds/checksums.

**Security** — mostly a set of links to the existing docs
(`security-model.md`, `threat-model.md`, `crypto.md`) plus 2-3 sentences of
plain-language framing. No duplicate source of truth for security claims.

**Privacy Policy** — accurate to what actually happens:
  * The website: no cookies, uses Vercel Analytics (cookie-less, no
    persistent identifiers, aggregate page-view counts only) — see §7.
  * The desktop app and extension: no telemetry, no crash reporting, no
    accounts, no analytics, master password and vault keys never leave the
    device unencrypted; if the user has a self-hosted server, it stores
    ciphertext plus routing metadata as documented in `server-sync.md`.
  * A line making clear the website operator (you) cannot see vault
    contents because there's no mechanism by which they'd reach it.

**Terms of Service** — standard, short: software provided as-is, no
warranty, MIT/Apache-2.0 dual license per the existing `LICENSE-MIT` /
`LICENSE-APACHE`, no SLA, self-hosting the server is the user's
responsibility per `docs/deployment.md`.

## 7. Analytics

Vercel Analytics (`@vercel/analytics`), which is cookie-less and reports
aggregate page views/paths without persistent per-visitor identifiers. This
is the one piece of third-party code on the site, and it's disclosed
plainly in the Privacy Policy. Nothing from the product itself (desktop app
or extension) is affected — this is scoped to `apps/web` only.

## 8. Download mechanism

`src/lib/releases.ts` fetches:

```text
GET https://api.github.com/repos/rochasamuel/havenkeys/releases/latest
```

client-side (no API token needed for public unauthenticated read access;
rate limit is 60 req/hr per IP, ample for a marketing page), and picks
matching assets by filename suffix (`.msi`, `.dmg`, `.AppImage`, `.deb`).
If the fetch fails (rate-limited, offline, no release yet) the Download
page falls back to a static "View releases on GitHub" link — never a dead
button.

## 9. Release pipeline

New `.github/workflows/release.yml`:

* Trigger: push of a tag matching `desktop-v*` (keeps desktop release tags
  distinct from any future tagging of the server or extension).
* Matrix: `windows-latest`, `macos-latest`, `ubuntu-latest`.
* Uses `tauri-apps/tauri-action` (the official, maintained GitHub Action
  for this exact job) to build `apps/desktop` on each runner and attach the
  resulting installers to a GitHub Release for that tag, creating the
  release if it doesn't exist.
* No binaries are ever committed into the git tree — "downloadable from
  GitHub" is satisfied by Release assets, which live in GitHub's release
  storage, not the repository's tracked files.
* Known limitation to document in the release workflow's own comments and
  in `docs/deployment.md`: the produced installers are unsigned. Windows
  SmartScreen and macOS Gatekeper will warn on first run. Signing
  certificates are a real cost/process and are explicitly out of scope for
  this MVP pass — noted, not solved.

## 10. Testing / verification

* `apps/web` gets the same `typecheck` script wired into the root `pnpm -r
  typecheck`.
* A small test for `releases.ts`'s asset-matching logic (given a fixture
  GitHub API response, picks the right asset per OS; falls back cleanly on
  a malformed/empty response).
* Manual verification before calling this done: `pnpm --filter
  @havenkeys/web build` succeeds, the site renders correctly against a real
  (or fixture) GitHub Release, and a Vercel preview deploy is checked in a
  browser.
* The release workflow itself is verified by actually pushing a
  `desktop-v0.1.0` tag (or similar) and confirming installers land on a
  GitHub Release — this is infrastructure, not something a unit test can
  cover.

## 11. Open follow-ups (explicitly out of scope here)

* Actually submitting to Chrome Web Store / Firefox Add-ons.
* Code-signing the desktop installers.
* A blog or changelog page.
* Any A/B testing, marketing pixels, or SEO tooling beyond basic meta tags.
