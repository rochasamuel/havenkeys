# The HavenKeys website (`apps/web`)

A static marketing/download site — no backend, no database. It talks only to
the public GitHub API (to find the latest release) and, once deployed, to
Vercel Analytics.

## Local development

```sh
pnpm --filter @havenkeys/web dev      # http://localhost:5173
pnpm --filter @havenkeys/web build    # typecheck + production build
pnpm --filter @havenkeys/web test     # vitest
```

## Deploying to Vercel

1. vercel.com → **Add New Project** → import `rochasamuel/havenkeys`.
2. **Root Directory:** `apps/web`. Framework preset: **Vite**. No
   environment variables are required.
3. Deploy. Vercel builds `apps/web` on every push to `main`.

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
