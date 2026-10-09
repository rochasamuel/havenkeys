# The HavenKeys website (`apps/web`)

A static marketing/download site — no backend, no database of its own. It talks
to the public GitHub API (to find the latest release), to `api.havenkeys.net`
from `/signup` (the only other host its CSP allows) and, once deployed, to
Vercel Analytics.

## Local development

```sh
pnpm --filter @havenkeys/web dev      # http://localhost:5173
pnpm --filter @havenkeys/web build    # typecheck + production build
pnpm --filter @havenkeys/web test     # vitest
pnpm ui:check --app=web               # the site in both languages, at phone and desktop widths
```

## Pages

Each page exists in English (`/…`) and Portuguese (`/pt-br/…`); the routes are
declared once in `apps/web/src/App.tsx`.

| Route | Page |
| --- | --- |
| `/` | Home: what HavenKeys is, written for everyday users |
| `/download` | Download: an account step first, then desktop, Android and the browser extension |
| `/signup` | Create account: email and terms, a six-digit code, then the setup code (kept in page memory only) with Copy and downloads |
| `/pricing` | Pricing: the Personal plan, the 14-day trial and what a frozen account keeps (price "coming soon") |
| `/security` | Security: what is protected and what is not, in plain language |
| `/self-host` | Self-host: run `havenkeys-server` yourself (Compose bundle, Railway) |
| `/developers` | Developers: the technical material (architecture, crypto, protocol) |
| `/privacy`, `/terms`, `/delete-account` | Legal pages |

* **Create account.** `/signup` is the only page that calls
  `api.havenkeys.net` (allowed in the CSP `connect-src`); every other page
  talks only to GitHub's public API and Vercel Analytics.
* **Railway button.** `RAILWAY_TEMPLATE_URL` in `apps/web/src/lib/links.ts` is
  empty until the Railway template is published (`deploy/railway/README.md`).
  Setting it to the template's URL turns the "Deploy on Railway" button on.
* **Self-hosting docs.** `docs/self-hosting.md` and `deploy/compose/` back the
  Self-host page.
* **Visual check.** `pnpm ui:check --app=web` renders the pages in both
  languages at phone and desktop widths (add `--no-build` to reuse a build).

## Languages

The site is in English (at `/`) and Brazilian Portuguese (under `/pt-br`).
Every visible string lives in `apps/web/src/i18n/en.tsx` and
`apps/web/src/i18n/pt-BR.tsx`; components never hard-code copy. The
Portuguese file is typed against the English one, so adding a string to
`en.tsx` without translating it fails `pnpm --filter @havenkeys/web typecheck`.

* The nav toggle and the footer switch to the same page in the other
  language, keeping any `#section`.
* A browser whose first language is Portuguese that lands on `/` is sent to
  `/pt-br` once. Picking a language with the switcher stores that choice in
  `localStorage` (`hk-locale`), and the redirect stops. Only `/` redirects, so
  shared links to English pages stay English.
* Names the app shows in English (Secret Key, Recovery Sheet,
  `havenkeys-server`) stay in English in the Portuguese copy. The Recovery Sheet
  illustration stays in English because it depicts the printed kit.
* The Portuguese pages carry a Portuguese translation of the audit disclaimer.

To add a language: add it to `LOCALES` in `src/i18n/locale.ts`, write a
dictionary typed as `Messages`, register it in `src/i18n/context.tsx`, and add
its routes in `src/App.tsx`.

## Deploying to Vercel

1. vercel.com → **Add New Project** → import `rochasamuel/havenkeys`.
2. **Root Directory:** `apps/web`. Framework preset: **Vite**. No
   environment variables are required.
3. Deploy. Vercel builds `apps/web` on every push to `main`.
4. **Analytics → Enable Web Analytics** in the Vercel project. Until it is
   enabled, the analytics script 404s and nothing is counted.

## Custom domain (`havenkeys.net`)

1. In the Vercel project → **Settings → Domains**, add `havenkeys.net` and
   `www.havenkeys.net`.
2. Vercel shows the exact DNS records to add at your registrar for that
   project (typically an `A` record for the apex and a `CNAME` for `www`,
   pointing at Vercel's edge). Add whatever Vercel's dashboard displays —
   it can differ slightly per account, so the dashboard is the source of
   truth, not a value copied from documentation.
3. Set `www.havenkeys.net` to redirect to the apex in the same Domains
   panel.

## Cutting a desktop release

The download page always links to the latest GitHub Release's assets, so
publishing one is the only step:

```sh
git tag desktop-v0.1.0
git push origin desktop-v0.1.0
```

This runs `.github/workflows/release.yml`, which builds the Windows, macOS
and Linux installers and attaches them to a **draft** GitHub Release for
that tag. Review the draft and click **Publish release** on GitHub — the
release stays a draft (not publicly visible, not linked from the download
page) until you do.

**Known limitation:** the installers are unsigned. Windows SmartScreen and
macOS Gatekeeper will warn on first run. Code-signing certificates are a
real ongoing cost and are out of scope for this MVP.

## Cutting an Android release

The download page links to the newest **published** release whose tag starts
with `android-v`. Android releases are never marked "latest", so the desktop
download and the desktop auto-updater keep pointing at desktop releases.

1. Raise `versionCode` and `versionName` in `apps/android/app/build.gradle.kts`
   (Android refuses an update whose `versionCode` is not higher), commit, push.
2. Tag and push: `git tag android-v0.1.0 && git push origin android-v0.1.0`.
3. `.github/workflows/android-release.yml` builds the release-mode Rust
   library and the signed APK, refuses an unsigned or debuggable APK, and
   attaches `HavenKeys-<version>.apk` and its `.sha256` to a **draft**
   release with the signing certificate's fingerprint in the notes.
4. Download the draft APK and install it on a phone. Then publish with
   `gh release edit android-v0.1.0 --draft=false --latest=false` (in the web
   form, untick "Set as the latest release": it is ticked by default). Confirm
   `gh release view --json tagName -q .tagName` still prints a `desktop-v`
   tag; if it prints `android-v`, the desktop download and auto-updater are
   broken until you mark the desktop release latest again.

If a run fails after the draft exists, delete the draft
(`gh release delete android-v0.1.0 --yes`) before re-running.

The signing key's custody is in `docs/android.md` → Release key custody.

## In-app updates (from 0.9.0)

From version 0.9.0, the desktop app updates itself on Windows, macOS and the
Linux AppImage: it checks GitHub Releases on its own, and offers a signed
update the user installs with one click (`docs/security-model.md` §18). `.deb`
and `.rpm` installs are not replaced in place — the app tells the user a new
version exists and links to the release page, where they download and install
it themselves. Anyone still on 0.8.0 or earlier has no updater at all and must
install 0.9.0 by hand, over the existing install; no uninstall is needed.
