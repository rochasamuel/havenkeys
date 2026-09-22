# Marketing/download website Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a static marketing/download website at `havenkeys.net` (new `apps/web` workspace member) with a Download page that always points at real installers, Privacy Policy and Terms of Service pages, and a GitHub Actions pipeline that builds and publishes those installers.

**Architecture:** A pure static Vite + React + TypeScript SPA (`apps/web`), styled entirely with the existing `@havenkeys/ui` design tokens (no new palette), deployed to Vercel. The Download page fetches the latest GitHub Release from the public GitHub API client-side and links directly to its assets, so it never goes stale. A new `.github/workflows/release.yml` builds Windows/macOS/Linux installers via `tauri-apps/tauri-action` on a `desktop-v*` tag and attaches them to a (draft) GitHub Release.

**Tech Stack:** Vite 8, React 19, TypeScript 7, react-router-dom 7, `@vercel/analytics`, vitest 5 — all versions matched to what `apps/desktop` already uses, for consistency across the workspace.

**Spec:** `docs/superpowers/specs/2026-09-22-marketing-website-design.md`

## Global Constraints

* No new design-token source of truth: colors/type/spacing come only from `@havenkeys/ui/tokens.css`, imported the same way `apps/desktop/src/main.tsx` does it (fonts, then tokens, then local stylesheet).
* No hype or absolute security claims anywhere on the site (no "military-grade," "unhackable," "100% secure"). The exact sentence `This software has not undergone an independent security audit and should not be considered a replacement for professionally audited password managers for high-value production use.` must appear on the homepage and in the footer.
* The site sets no cookies. The only third-party code is `@vercel/analytics`, which is cookie-less; this is disclosed in the Privacy Policy.
* No installer binaries are ever committed into the git tree. Downloads are served exclusively via GitHub Release assets.
* GitHub repo is `rochasamuel/havenkeys`. Desktop release tags use the scheme `desktop-v*` (e.g. `desktop-v0.1.0`), kept distinct from any future server/extension tags.
* Domain is `havenkeys.net` (apex), with `www.havenkeys.net` redirecting to it, hosted on Vercel.
* This is UI work: per project convention, verify with `pnpm --filter @havenkeys/web build` (typecheck + build) after every task, and do a real-browser check (`pnpm --filter @havenkeys/web dev`, open the route) before considering a content task done. If no browser is available in the execution environment, say so explicitly rather than claiming visual verification.

---

### Task 1: Scaffold the `apps/web` workspace

**Files:**
- Create: `apps/web/package.json`
- Create: `apps/web/tsconfig.json`
- Create: `apps/web/vite.config.ts`
- Create: `apps/web/index.html`
- Create: `apps/web/vercel.json`
- Create: `apps/web/public/favicon.ico` (copied from the desktop app's icon)
- Create: `apps/web/src/main.tsx`
- Create: `apps/web/src/App.tsx`
- Modify: `package.json:6` (root) — add a `dev:web` convenience script

**Interfaces:**
- Produces: `App` — a named export from `apps/web/src/App.tsx`, a React component with no props. Every later task that touches routing modifies this file.

- [ ] **Step 1: Create `apps/web/package.json`**

```json
{
  "name": "@havenkeys/web",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "preview": "vite preview",
    "typecheck": "tsc --noEmit",
    "test": "vitest run"
  },
  "dependencies": {
    "@fontsource-variable/hanken-grotesk": "^5.3.0",
    "@fontsource/jetbrains-mono": "^5.3.0",
    "@havenkeys/ui": "workspace:*",
    "react": "^19.3.0",
    "react-dom": "^19.3.0"
  },
  "devDependencies": {
    "@types/react": "^19.3.0",
    "@types/react-dom": "^19.3.0",
    "@vitejs/plugin-react": "^6.1.1",
    "typescript": "^7.0.2",
    "vite": "^8.3.0",
    "vitest": "^5.0.1"
  }
}
```

- [ ] **Step 2: Create `apps/web/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "isolatedModules": true,
    "skipLibCheck": true,
    "noEmit": true,
    "types": ["vite/client"]
  },
  "include": ["src", "vite.config.ts"]
}
```

- [ ] **Step 3: Create `apps/web/vite.config.ts`**

```ts
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    target: "es2022",
    sourcemap: false,
  },
  test: {
    environment: "node",
  },
});
```

- [ ] **Step 4: Create `apps/web/index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta
      name="description"
      content="HavenKeys is a local-first password manager: a desktop app with a Rust security core, a browser extension, and a server you run yourself."
    />
    <title>HavenKeys — a password manager you actually own</title>
    <link rel="icon" href="/favicon.ico" />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5: Create `apps/web/vercel.json`**

```json
{
  "rewrites": [{ "source": "/(.*)", "destination": "/index.html" }]
}
```

- [ ] **Step 6: Copy the existing app icon as the site favicon**

```bash
mkdir -p apps/web/public
cp apps/desktop/src-tauri/icons/icon.ico apps/web/public/favicon.ico
```

- [ ] **Step 7: Create `apps/web/src/App.tsx` (placeholder, replaced in Task 3)**

```tsx
export function App() {
  return <p>HavenKeys — coming soon.</p>;
}
```

- [ ] **Step 8: Create `apps/web/src/main.tsx`**

```tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/hanken-grotesk";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
// Tokens first: everything below reads them as custom properties.
import "@havenkeys/ui/tokens.css";
import { App } from "./App";

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
```

- [ ] **Step 9: Add a `dev:web` convenience script to the root `package.json`**

Add this line inside the root `"scripts"` object (after `"build:extension"`):

```json
    "dev:web": "pnpm --filter @havenkeys/web dev",
```

- [ ] **Step 10: Install and verify the build**

```bash
pnpm install
pnpm --filter @havenkeys/web build
```

Expected: installs cleanly, `tsc --noEmit` reports no errors, `vite build` produces `apps/web/dist/`.

- [ ] **Step 11: Commit**

```bash
git add apps/web package.json pnpm-lock.yaml
git commit -m "feat(web): scaffold the marketing site workspace"
```

---

### Task 2: Latest-release lookup (`releases.ts`)

**Files:**
- Create: `apps/web/src/lib/releases.ts`
- Test: `apps/web/src/lib/releases.test.ts`

**Interfaces:**
- Produces: `type ReleaseAsset = { name: string; browser_download_url: string }`, `type Platform = "windows" | "macos" | "linux-appimage" | "linux-deb"`, `type LatestRelease = { tag_name: string; html_url: string; assets: ReleaseAsset[] }`, `function pickAsset(assets: ReleaseAsset[], platform: Platform): ReleaseAsset | null`, `function fetchLatestRelease(): Promise<LatestRelease | null>`, `const RELEASES_PAGE_URL: string`. The Download page (Task 4) imports all of these.

- [ ] **Step 1: Write the failing tests**

Create `apps/web/src/lib/releases.test.ts`:

```ts
import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchLatestRelease, pickAsset, type ReleaseAsset } from "./releases";

describe("pickAsset", () => {
  const assets: ReleaseAsset[] = [
    { name: "HavenKeys_0.1.0_x64-setup.exe", browser_download_url: "https://example.com/exe" },
    { name: "HavenKeys_0.1.0_x64_en-US.msi", browser_download_url: "https://example.com/msi" },
    { name: "HavenKeys_0.1.0_aarch64.dmg", browser_download_url: "https://example.com/dmg" },
    {
      name: "HavenKeys_0.1.0_amd64.AppImage",
      browser_download_url: "https://example.com/appimage",
    },
    { name: "HavenKeys_0.1.0_amd64.deb", browser_download_url: "https://example.com/deb" },
  ];

  it("prefers the .msi over the .exe for Windows", () => {
    expect(pickAsset(assets, "windows")?.name).toBe("HavenKeys_0.1.0_x64_en-US.msi");
  });

  it("finds the macOS .dmg", () => {
    expect(pickAsset(assets, "macos")?.name).toBe("HavenKeys_0.1.0_aarch64.dmg");
  });

  it("finds the Linux AppImage", () => {
    expect(pickAsset(assets, "linux-appimage")?.name).toBe("HavenKeys_0.1.0_amd64.AppImage");
  });

  it("finds the Linux .deb package", () => {
    expect(pickAsset(assets, "linux-deb")?.name).toBe("HavenKeys_0.1.0_amd64.deb");
  });

  it("returns null when no asset matches the platform", () => {
    expect(pickAsset([], "windows")).toBeNull();
  });
});

describe("fetchLatestRelease", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("returns the parsed release on a successful response", async () => {
    const release = { tag_name: "desktop-v0.1.0", html_url: "https://x", assets: [] };
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, json: async () => release }),
    );
    expect(await fetchLatestRelease()).toEqual(release);
  });

  it("returns null on a non-ok response", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: false }));
    expect(await fetchLatestRelease()).toBeNull();
  });

  it("returns null when the response has no assets array", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, json: async () => ({ tag_name: "x" }) }),
    );
    expect(await fetchLatestRelease()).toBeNull();
  });

  it("returns null when the fetch throws", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("offline")));
    expect(await fetchLatestRelease()).toBeNull();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
pnpm --filter @havenkeys/web test
```

Expected: FAIL — `./releases` does not exist yet.

- [ ] **Step 3: Implement `apps/web/src/lib/releases.ts`**

```ts
export interface ReleaseAsset {
  name: string;
  browser_download_url: string;
}

export interface LatestRelease {
  tag_name: string;
  html_url: string;
  assets: ReleaseAsset[];
}

export type Platform = "windows" | "macos" | "linux-appimage" | "linux-deb";

// Order matters: the first matching suffix wins, so the preferred installer
// format for a platform (e.g. .msi over the NSIS .exe) is listed first.
const PLATFORM_SUFFIXES: Record<Platform, string[]> = {
  windows: [".msi", ".exe"],
  macos: [".dmg"],
  "linux-appimage": [".AppImage"],
  "linux-deb": [".deb"],
};

export function pickAsset(assets: ReleaseAsset[], platform: Platform): ReleaseAsset | null {
  for (const suffix of PLATFORM_SUFFIXES[platform]) {
    const match = assets.find((asset) => asset.name.endsWith(suffix));
    if (match) return match;
  }
  return null;
}

const RELEASES_API_URL = "https://api.github.com/repos/rochasamuel/havenkeys/releases/latest";
export const RELEASES_PAGE_URL = "https://github.com/rochasamuel/havenkeys/releases";

export async function fetchLatestRelease(): Promise<LatestRelease | null> {
  try {
    const response = await fetch(RELEASES_API_URL, {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!response.ok) return null;
    const data = (await response.json()) as LatestRelease;
    if (!Array.isArray(data.assets)) return null;
    return data;
  } catch {
    return null;
  }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
pnpm --filter @havenkeys/web test
```

Expected: all tests in `releases.test.ts` PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/lib
git commit -m "feat(web): add latest-release lookup for the download page"
```

---

### Task 3: Design system shell — global styles, Nav, Footer, Home page

**Files:**
- Create: `apps/web/src/styles/global.css`
- Create: `apps/web/src/components/Nav.tsx`
- Create: `apps/web/src/components/Footer.tsx`
- Create: `apps/web/src/pages/Home.tsx`
- Modify: `apps/web/src/App.tsx` (replaces the Task 1 placeholder)
- Modify: `apps/web/src/main.tsx` (imports the new stylesheet)

**Interfaces:**
- Consumes: nothing from earlier tasks (Home doesn't need `releases.ts`).
- Produces: `Nav`, `Footer`, `Home` — named exports, no props, from their respective files. `App.tsx`'s `<Routes>` block is what Tasks 4–7 each add one `<Route>` to.

- [ ] **Step 1: Create `apps/web/src/styles/global.css`**

```css
/* HavenKeys marketing site — layout only.
   Colors, type and spacing come from @havenkeys/ui/tokens.css, loaded by
   main.tsx before this file. */

:root {
  font-family: var(--font);
  font-size: 16px;
  line-height: var(--leading-base);
  color: var(--text);
  background: var(--bg);
  -webkit-font-smoothing: antialiased;
}

*,
*::before,
*::after {
  box-sizing: border-box;
}

body {
  margin: 0;
}

a {
  color: var(--brass-hi);
}

code {
  font-family: var(--mono);
  background: var(--raised);
  padding: 0.1em 0.4em;
  border-radius: var(--r-xs);
}

.button {
  display: inline-block;
  padding: 0.75em 1.5em;
  border-radius: var(--r-md);
  font-weight: var(--weight-control);
  text-decoration: none;
}

.button--primary {
  background: var(--primary-bg);
  color: var(--primary-fg);
}

.button--primary:hover {
  background: var(--primary-hover);
}

.button--secondary {
  border: 1px solid var(--line-strong);
  color: var(--text);
}

.nav {
  border-bottom: 1px solid var(--line);
  position: sticky;
  top: 0;
  background: var(--bg);
  z-index: var(--z-page-top);
}

.nav__inner {
  max-width: 1080px;
  margin: 0 auto;
  padding: var(--space-3xl) var(--space-4xl);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.nav__brand {
  font-weight: var(--weight-brand);
  font-size: var(--text-3xl);
  color: var(--text-strong);
  text-decoration: none;
}

.nav__links {
  display: flex;
  align-items: center;
  gap: var(--space-4xl);
}

.nav__links a {
  color: var(--text);
  text-decoration: none;
}

.nav__cta {
  padding: var(--space-sm) var(--space-2xl);
  border-radius: var(--r-md);
  background: var(--primary-bg);
  color: var(--primary-fg) !important;
}

.hero {
  max-width: 720px;
  margin: 0 auto;
  padding: 96px var(--space-4xl) var(--space-5xl);
}

.hero__eyebrow {
  text-transform: uppercase;
  letter-spacing: var(--tracking-caps);
  color: var(--brass);
  font-size: var(--text-sm);
  font-weight: var(--weight-label);
}

.hero h1 {
  font-size: 40px;
  line-height: var(--leading-tight);
  color: var(--text-strong);
  margin: var(--space-3xl) 0;
}

.hero__lede {
  font-size: var(--text-xl);
  color: var(--muted);
  max-width: 60ch;
}

.hero__actions {
  display: flex;
  gap: var(--space-2xl);
  margin-top: var(--space-4xl);
}

.how-it-works,
.features,
.security-teaser {
  max-width: 1080px;
  margin: 0 auto;
  padding: var(--space-5xl) var(--space-4xl);
}

.how-it-works ol {
  max-width: 640px;
  padding-left: 1.25em;
  color: var(--muted);
}

.how-it-works li {
  margin-bottom: var(--space-2xl);
}

.features__grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: var(--space-3xl);
  margin-top: var(--space-4xl);
}

.feature-card {
  border: 1px solid var(--line);
  border-radius: var(--r-lg);
  padding: var(--space-4xl);
  background: var(--raised);
}

.feature-card h3 {
  color: var(--text-strong);
  margin-top: 0;
}

.feature-card p {
  color: var(--muted);
}

.docs-page {
  max-width: 720px;
  margin: 0 auto;
  padding: var(--space-5xl) var(--space-4xl);
}

.docs-page h1 {
  color: var(--text-strong);
}

.docs-page__updated {
  color: var(--muted);
  font-size: var(--text-sm);
}

.docs-list {
  list-style: none;
  padding: 0;
}

.docs-list li {
  border-top: 1px solid var(--line);
  padding: var(--space-3xl) 0;
}

.docs-list a {
  font-weight: var(--weight-emphasis);
  font-size: var(--text-xl);
}

.download {
  max-width: 1080px;
  margin: 0 auto;
  padding: var(--space-5xl) var(--space-4xl);
}

.download__status {
  color: var(--muted);
}

.download__grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: var(--space-3xl);
  margin-top: var(--space-4xl);
}

.download__card {
  border: 1px solid var(--line);
  border-radius: var(--r-lg);
  padding: var(--space-4xl);
  background: var(--raised);
  text-align: center;
}

.download__button {
  display: inline-block;
  margin-top: var(--space-3xl);
  padding: var(--space-md) var(--space-3xl);
  border-radius: var(--r-md);
  background: var(--primary-bg);
  color: var(--primary-fg);
  text-decoration: none;
  font-weight: var(--weight-control);
}

.download__button--secondary {
  background: transparent;
  border: 1px solid var(--line-strong);
  color: var(--text);
}

.footer {
  border-top: 1px solid var(--line);
  margin-top: 96px;
}

.footer__inner {
  max-width: 1080px;
  margin: 0 auto;
  padding: var(--space-4xl);
  color: var(--muted);
  font-size: var(--text-sm);
}

.footer__disclaimer {
  max-width: 60ch;
}

.footer__links {
  display: flex;
  gap: var(--space-3xl);
  margin: var(--space-3xl) 0;
}

.footer__links a {
  color: var(--muted);
}

@media (max-width: 720px) {
  .features__grid,
  .download__grid {
    grid-template-columns: 1fr;
  }

  .nav__links {
    gap: var(--space-2xl);
  }
}
```

- [ ] **Step 2: Create `apps/web/src/components/Nav.tsx`**

```tsx
import { Link } from "react-router-dom";

export function Nav() {
  return (
    <header className="nav">
      <div className="nav__inner">
        <Link to="/" className="nav__brand">
          HavenKeys
        </Link>
        <nav className="nav__links">
          <Link to="/security">Security</Link>
          <a href="https://github.com/rochasamuel/havenkeys" target="_blank" rel="noreferrer">
            GitHub
          </a>
          <Link to="/download" className="nav__cta">
            Download
          </Link>
        </nav>
      </div>
    </header>
  );
}
```

- [ ] **Step 3: Create `apps/web/src/components/Footer.tsx`**

```tsx
import { Link } from "react-router-dom";

export function Footer() {
  return (
    <footer className="footer">
      <div className="footer__inner">
        <p className="footer__disclaimer">
          This software has not undergone an independent security audit and should not be
          considered a replacement for professionally audited password managers for high-value
          production use.
        </p>
        <div className="footer__links">
          <a href="https://github.com/rochasamuel/havenkeys" target="_blank" rel="noreferrer">
            GitHub
          </a>
          <a
            href="https://github.com/rochasamuel/havenkeys/blob/main/LICENSE-MIT"
            target="_blank"
            rel="noreferrer"
          >
            License
          </a>
          <Link to="/privacy">Privacy</Link>
          <Link to="/terms">Terms</Link>
        </div>
        <p>© {new Date().getFullYear()} HavenKeys</p>
      </div>
    </footer>
  );
}
```

- [ ] **Step 4: Create `apps/web/src/pages/Home.tsx`**

```tsx
import { Link } from "react-router-dom";

const FEATURES = [
  {
    title: "Logins & autofill",
    body: "Save usernames and passwords once. The browser extension suggests them only on the sites they were saved for — never automatically, never on a look-alike domain.",
  },
  {
    title: "Secure notes",
    body: "Arbitrary encrypted text for anything that isn't a login: recovery codes, passphrases, whatever needs to stay off a screenshot.",
  },
  {
    title: "TOTP codes",
    body: "Import an otpauth:// URI once; HavenKeys generates the six- or eight-digit code from then on, in Rust, without ever handing the secret to a webpage.",
  },
  {
    title: "Password generator",
    body: "Cryptographically random and unbiased, generated in Rust — length and character sets are yours to configure.",
  },
  {
    title: "Your own server",
    body: "havenkeys-server is a small server you run yourself. It stores ciphertext it cannot open, and it's the only thing keeping your devices in sync.",
  },
  {
    title: "Browser extension",
    body: "Manifest V3, minimal permissions, and every fill request checked against the current page's origin in Rust — not trusted from the extension.",
  },
];

export function Home() {
  return (
    <>
      <section className="hero">
        <p className="hero__eyebrow">Local-first password manager</p>
        <h1>Your passwords, encrypted before they ever leave your device.</h1>
        <p className="hero__lede">
          HavenKeys pairs a Tauri desktop app with a Rust security core, a browser extension, and
          a small server you run yourself. Keys and plaintext never leave your device — the
          server only ever holds ciphertext it has no way to open.
        </p>
        <div className="hero__actions">
          <Link to="/download" className="button button--primary">
            Download HavenKeys
          </Link>
          <a
            className="button button--secondary"
            href="https://github.com/rochasamuel/havenkeys"
            target="_blank"
            rel="noreferrer"
          >
            View on GitHub
          </a>
        </div>
      </section>

      <section className="how-it-works">
        <h2>How it's built</h2>
        <ol>
          <li>
            Your master password runs through <code>Argon2id</code> to derive a master key — it
            is never used as an encryption key directly.
          </li>
          <li>
            The master key unwraps a key-encryption key, which unwraps a randomly generated vault
            key.
          </li>
          <li>
            Every item is encrypted individually with <code>AES-256-GCM</code> under the vault
            key, with a unique nonce every time.
          </li>
          <li>
            The desktop UI never does cryptography and never sees your master password after
            unlock — that stays in the Rust core.
          </li>
        </ol>
      </section>

      <section className="features">
        <h2>What it does</h2>
        <div className="features__grid">
          {FEATURES.map((feature) => (
            <div className="feature-card" key={feature.title}>
              <h3>{feature.title}</h3>
              <p>{feature.body}</p>
            </div>
          ))}
        </div>
      </section>

      <section className="security-teaser">
        <h2>Security, not marketing</h2>
        <p>
          We'd rather point you at the actual documents than summarize them: the{" "}
          <Link to="/security">security page</Link> links straight to the threat model, the
          security model, and the cryptography design.
        </p>
        <p>
          This software has not undergone an independent security audit and should not be
          considered a replacement for professionally audited password managers for high-value
          production use.
        </p>
      </section>
    </>
  );
}
```

- [ ] **Step 5: Replace `apps/web/src/App.tsx`**

```tsx
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { Nav } from "./components/Nav";
import { Footer } from "./components/Footer";
import { Home } from "./pages/Home";

export function App() {
  return (
    <BrowserRouter>
      <Nav />
      <main>
        <Routes>
          <Route path="/" element={<Home />} />
        </Routes>
      </main>
      <Footer />
    </BrowserRouter>
  );
}
```

- [ ] **Step 6: Update `apps/web/src/main.tsx`**

Add the global stylesheet import after the tokens import:

```tsx
import "@havenkeys/ui/tokens.css";
import "./styles/global.css";
```

- [ ] **Step 7: Add the new dependency and verify**

Add `"react-router-dom": "^7.18.4"` to `apps/web/package.json`'s `"dependencies"`, then:

```bash
pnpm install
pnpm --filter @havenkeys/web build
```

Expected: no type errors, build succeeds.

- [ ] **Step 8: Manual browser check**

```bash
pnpm --filter @havenkeys/web dev
```

Open `http://localhost:5173/` and confirm the hero, "How it's built" steps, feature grid, and footer disclaimer render with the dark green/brass theme. If no browser is reachable in this environment, say so explicitly instead of claiming this step passed.

- [ ] **Step 9: Commit**

```bash
git add apps/web package.json pnpm-lock.yaml
git commit -m "feat(web): add design shell (nav, footer, styles) and the homepage"
```

---

### Task 4: Download page

**Files:**
- Create: `apps/web/src/pages/Download.tsx`
- Modify: `apps/web/src/App.tsx` — add the `/download` route
- Modify: `apps/web/src/components/Nav.tsx` — no change needed, the link already exists

**Interfaces:**
- Consumes: `fetchLatestRelease`, `pickAsset`, `RELEASES_PAGE_URL`, `LatestRelease`, `Platform` from `../lib/releases` (Task 2).

- [ ] **Step 1: Create `apps/web/src/pages/Download.tsx`**

```tsx
import { useEffect, useState } from "react";
import {
  fetchLatestRelease,
  pickAsset,
  RELEASES_PAGE_URL,
  type LatestRelease,
  type Platform,
} from "../lib/releases";

const PLATFORMS: Array<{ id: Platform; label: string; format: string }> = [
  { id: "windows", label: "Windows", format: ".msi installer" },
  { id: "macos", label: "macOS", format: ".dmg disk image" },
  { id: "linux-appimage", label: "Linux", format: ".AppImage" },
  { id: "linux-deb", label: "Linux (Debian/Ubuntu)", format: ".deb package" },
];

type LoadState =
  | { status: "loading" }
  | { status: "ready"; release: LatestRelease }
  | { status: "unavailable" };

export function Download() {
  const [state, setState] = useState<LoadState>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    fetchLatestRelease().then((release) => {
      if (cancelled) return;
      setState(release ? { status: "ready", release } : { status: "unavailable" });
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <section className="download">
      <h1>Download HavenKeys</h1>
      {state.status === "ready" && (
        <p className="download__status">Latest version: {state.release.tag_name}</p>
      )}
      {state.status === "loading" && (
        <p className="download__status">Checking latest release…</p>
      )}
      {state.status === "unavailable" && (
        <p className="download__status">
          Couldn't reach GitHub to find the latest build.{" "}
          <a href={RELEASES_PAGE_URL} target="_blank" rel="noreferrer">
            View releases on GitHub
          </a>
          .
        </p>
      )}
      <div className="download__grid">
        {PLATFORMS.map((platform) => {
          const asset =
            state.status === "ready" ? pickAsset(state.release.assets, platform.id) : null;
          return (
            <div className="download__card" key={platform.id}>
              <h2>{platform.label}</h2>
              <p>{platform.format}</p>
              {asset ? (
                <a className="download__button" href={asset.browser_download_url}>
                  Download for {platform.label}
                </a>
              ) : (
                <a
                  className="download__button download__button--secondary"
                  href={RELEASES_PAGE_URL}
                  target="_blank"
                  rel="noreferrer"
                >
                  View releases
                </a>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Add the route in `apps/web/src/App.tsx`**

```tsx
import { Download } from "./pages/Download";
```

and inside `<Routes>`, after the `"/"` route:

```tsx
          <Route path="/download" element={<Download />} />
```

- [ ] **Step 3: Verify**

```bash
pnpm --filter @havenkeys/web build
```

Expected: no type errors, build succeeds.

- [ ] **Step 4: Manual browser check**

```bash
pnpm --filter @havenkeys/web dev
```

Open `http://localhost:5173/download` and confirm it shows "Checking latest release…" then either real per-OS download buttons or the GitHub fallback link (there may be no release yet, since Task 10 hasn't shipped the workflow — the fallback link is the expected, correct state right now). If no browser is reachable, say so explicitly.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src
git commit -m "feat(web): add the download page"
```

---

### Task 5: Security page

**Files:**
- Create: `apps/web/src/pages/Security.tsx`
- Modify: `apps/web/src/App.tsx` — add the `/security` route

- [ ] **Step 1: Create `apps/web/src/pages/Security.tsx`**

```tsx
const DOCS = [
  {
    title: "Threat model",
    href: "https://github.com/rochasamuel/havenkeys/blob/main/docs/threat-model.md",
    body: "What HavenKeys defends against, and what it explicitly doesn't.",
  },
  {
    title: "Security model",
    href: "https://github.com/rochasamuel/havenkeys/blob/main/docs/security-model.md",
    body: "How the defenses in the threat model are actually enforced, permission by permission.",
  },
  {
    title: "Cryptography",
    href: "https://github.com/rochasamuel/havenkeys/blob/main/docs/crypto.md",
    body: "The key hierarchy, the encrypted-blob format, and the exact Argon2id and AES-256-GCM parameters.",
  },
  {
    title: "Security review",
    href: "https://github.com/rochasamuel/havenkeys/blob/main/docs/security-review.md",
    body: "Findings from reviewing this implementation against its own threat model, including what's still open.",
  },
];

export function Security() {
  return (
    <section className="docs-page">
      <h1>Security</h1>
      <p>
        HavenKeys has not undergone an independent security audit. Rather than restate its design
        here, these are the actual documents that describe it — read them before trusting it with
        anything.
      </p>
      <ul className="docs-list">
        {DOCS.map((doc) => (
          <li key={doc.href}>
            <a href={doc.href} target="_blank" rel="noreferrer">
              {doc.title}
            </a>
            <p>{doc.body}</p>
          </li>
        ))}
      </ul>
    </section>
  );
}
```

- [ ] **Step 2: Add the route in `apps/web/src/App.tsx`**

```tsx
import { Security } from "./pages/Security";
```

```tsx
          <Route path="/security" element={<Security />} />
```

- [ ] **Step 3: Verify**

```bash
pnpm --filter @havenkeys/web build
```

- [ ] **Step 4: Manual browser check**

Open `http://localhost:5173/security` with `pnpm --filter @havenkeys/web dev` running, confirm the four doc links render and point at real GitHub URLs. If no browser is reachable, say so explicitly.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src
git commit -m "feat(web): add the security page"
```

---

### Task 6: Privacy Policy page

**Files:**
- Create: `apps/web/src/pages/Privacy.tsx`
- Modify: `apps/web/src/App.tsx` — add the `/privacy` route

- [ ] **Step 1: Create `apps/web/src/pages/Privacy.tsx`**

```tsx
export function Privacy() {
  return (
    <section className="docs-page">
      <h1>Privacy Policy</h1>
      <p className="docs-page__updated">Last updated September 22, 2026.</p>

      <h2>This website</h2>
      <p>
        havenkeys.net sets no cookies. It uses Vercel Analytics, which reports which pages were
        viewed in aggregate without a persistent identifier or cross-site tracking. There is no
        advertising and no third-party tracking script of any kind.
      </p>

      <h2>The desktop app and browser extension</h2>
      <p>
        HavenKeys the application contains no telemetry, no analytics, no crash reporting, and no
        accounts system in this version. Your master password never leaves your device and is
        never transmitted anywhere, including to the browser extension. Vault keys are generated
        on your device.
      </p>
      <p>
        If you choose to run <code>havenkeys-server</code> yourself and connect a device to it,
        that server stores encrypted vault data it cannot decrypt, plus the minimum routing
        metadata described in the{" "}
        <a
          href="https://github.com/rochasamuel/havenkeys/blob/main/docs/server-sync.md"
          target="_blank"
          rel="noreferrer"
        >
          server-sync design
        </a>
        . That server is one you host and control — we do not operate a hosted version and have
        no access to it.
      </p>

      <h2>What we never collect</h2>
      <ul>
        <li>Master passwords</li>
        <li>Vault encryption keys</li>
        <li>Stored passwords, usernames, TOTP secrets, or secure notes</li>
        <li>Browsing history or the contents of pages you visit</li>
      </ul>

      <h2>Contact</h2>
      <p>
        Questions about this policy can be opened as an issue on{" "}
        <a href="https://github.com/rochasamuel/havenkeys/issues" target="_blank" rel="noreferrer">
          GitHub
        </a>
        .
      </p>
    </section>
  );
}
```

- [ ] **Step 2: Add the route in `apps/web/src/App.tsx`**

```tsx
import { Privacy } from "./pages/Privacy";
```

```tsx
          <Route path="/privacy" element={<Privacy />} />
```

- [ ] **Step 3: Verify**

```bash
pnpm --filter @havenkeys/web build
```

- [ ] **Step 4: Manual browser check**

Open `http://localhost:5173/privacy`, confirm it renders. If no browser is reachable, say so explicitly.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src
git commit -m "feat(web): add the privacy policy page"
```

---

### Task 7: Terms of Service page

**Files:**
- Create: `apps/web/src/pages/Terms.tsx`
- Modify: `apps/web/src/App.tsx` — add the `/terms` route

- [ ] **Step 1: Create `apps/web/src/pages/Terms.tsx`**

```tsx
export function Terms() {
  return (
    <section className="docs-page">
      <h1>Terms of Service</h1>
      <p className="docs-page__updated">Last updated September 22, 2026.</p>

      <h2>License</h2>
      <p>
        HavenKeys is open-source software, dual-licensed under the{" "}
        <a
          href="https://github.com/rochasamuel/havenkeys/blob/main/LICENSE-MIT"
          target="_blank"
          rel="noreferrer"
        >
          MIT License
        </a>{" "}
        and the{" "}
        <a
          href="https://github.com/rochasamuel/havenkeys/blob/main/LICENSE-APACHE"
          target="_blank"
          rel="noreferrer"
        >
          Apache License 2.0
        </a>
        . You may use, modify, and redistribute it under the terms of either.
      </p>

      <h2>No warranty</h2>
      <p>
        HavenKeys is provided "as is," without warranty of any kind, express or implied,
        including but not limited to fitness for a particular purpose. This software has not
        undergone an independent security audit and should not be considered a replacement for
        professionally audited password managers for high-value production use. You use it at
        your own risk.
      </p>

      <h2>Self-hosting</h2>
      <p>
        If you run <code>havenkeys-server</code> yourself, you are solely responsible for its
        deployment, its uptime, and its backups. The server is the authoritative copy of your
        vault; losing it without a tested backup means losing your data. See{" "}
        <a
          href="https://github.com/rochasamuel/havenkeys/blob/main/docs/deployment.md"
          target="_blank"
          rel="noreferrer"
        >
          the deployment guide
        </a>
        , particularly its section on backups, before storing anything you can't afford to lose.
      </p>

      <h2>No service, no account</h2>
      <p>
        We do not operate a hosted version of HavenKeys and do not maintain accounts on your
        behalf. There is no subscription, no SLA, and no support obligation implied by
        downloading this software.
      </p>
    </section>
  );
}
```

- [ ] **Step 2: Add the route in `apps/web/src/App.tsx`**

```tsx
import { Terms } from "./pages/Terms";
```

```tsx
          <Route path="/terms" element={<Terms />} />
```

- [ ] **Step 3: Verify**

```bash
pnpm --filter @havenkeys/web build
```

- [ ] **Step 4: Manual browser check**

Open `http://localhost:5173/terms`, confirm it renders. If no browser is reachable, say so explicitly.

- [ ] **Step 5: Commit**

```bash
git add apps/web/src
git commit -m "feat(web): add the terms of service page"
```

---

### Task 8: Vercel Analytics

**Files:**
- Modify: `apps/web/package.json` — add `@vercel/analytics`
- Modify: `apps/web/src/App.tsx` — render `<Analytics />`

- [ ] **Step 1: Add the dependency**

Add `"@vercel/analytics": "^2.0.1"` to `apps/web/package.json`'s `"dependencies"`, then:

```bash
pnpm install
```

- [ ] **Step 2: Wire it into `apps/web/src/App.tsx`**

```tsx
import { Analytics } from "@vercel/analytics/react";
```

Render it as the last child inside `<BrowserRouter>`, after `<Footer />`:

```tsx
      <Footer />
      <Analytics />
```

- [ ] **Step 3: Verify**

```bash
pnpm --filter @havenkeys/web build
```

Expected: no type errors, build succeeds. (`<Analytics />` is a no-op outside a Vercel deployment, so nothing observable changes locally — this step only confirms it compiles and doesn't crash the app.)

- [ ] **Step 4: Commit**

```bash
git add apps/web/package.json pnpm-lock.yaml apps/web/src/App.tsx
git commit -m "feat(web): add cookie-less Vercel Analytics"
```

---

### Task 9: Deployment docs

**Files:**
- Create: `docs/website.md`
- Modify: `README.md` — one row in the Status table

**Interfaces:** none (documentation only).

- [ ] **Step 1: Create `docs/website.md`**

```markdown
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
```

- [ ] **Step 2: Add a row to `README.md`'s Status table**

In the table under `## Status`, after the `Account server (havenkeys-server)` row, add:

```markdown
| Marketing/download website (havenkeys.net) | Implemented; static site on Vercel, download page reads GitHub Releases ([docs/website.md](docs/website.md)) |
```

- [ ] **Step 3: Commit**

```bash
git add docs/website.md README.md
git commit -m "docs: document the website's deployment and release process"
```

---

### Task 10: Desktop release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create `.github/workflows/release.yml`**

```yaml
name: release

on:
  push:
    tags:
      - "desktop-v*"

jobs:
  release:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: windows-latest
          - platform: macos-latest
          - platform: ubuntu-latest
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4

      - name: Install Linux dependencies
        if: matrix.platform == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install pnpm
        uses: pnpm/action-setup@v4

      - name: Install Node
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm

      - name: Install workspace dependencies
        run: pnpm install --frozen-lockfile

      - name: Build and attach installers
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          projectPath: apps/desktop
          tagName: ${{ github.ref_name }}
          releaseName: "HavenKeys ${{ github.ref_name }}"
          releaseBody: "Installers for this release. Unsigned: Windows SmartScreen and macOS Gatekeeper will warn on first run."
          releaseDraft: true
          prerelease: false
```

- [ ] **Step 2: Validate the YAML syntax**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))" && echo "valid YAML"
```

Expected: `valid YAML`. (Actually exercising this workflow means pushing a real tag, which creates a public-facing GitHub Actions run and a draft Release — that's a decision for you to make when there's a real version to ship, not something to do as part of this plan.)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: build and publish desktop installers on a release tag"
```

---

### Task 11: Final workspace verification

**Files:** none — verification only.

- [ ] **Step 1: Typecheck and test the whole workspace**

```bash
pnpm -r typecheck
pnpm -r test
```

Expected: all packages, including `@havenkeys/web`, pass.

- [ ] **Step 2: Full production build**

```bash
pnpm --filter @havenkeys/web build
```

Expected: succeeds with no errors.

- [ ] **Step 3: Manual pass over every route**

```bash
pnpm --filter @havenkeys/web dev
```

With it running, open each of `/`, `/download`, `/security`, `/privacy`,
`/terms` in a real browser and confirm: the nav and footer appear
consistently, the audit disclaimer text is present, no route is blank, and
the Download page shows either real installer buttons or the GitHub
fallback link. If no browser is reachable in this environment, say so
explicitly rather than reporting this as verified.

- [ ] **Step 4: Run the repository's dependency audit**

```bash
pnpm audit
```

Investigate and address anything it flags in the new dependencies
(`react-router-dom`, `@vercel/analytics`) before considering this plan
done; do not silently ignore findings.

- [ ] **Step 5: Commit (only if any of the above required fixes)**

```bash
git add -A
git commit -m "fix(web): address workspace verification findings"
```
