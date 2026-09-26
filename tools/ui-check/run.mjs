#!/usr/bin/env node
// Layout check for both languages: renders every extension page state and
// every desktop screen in English and Brazilian Portuguese, light and dark,
// at their real sizes, saves screenshots to ui-check-output/, and fails when
// UI text is clipped or anything spills out of the viewport horizontally
// (see check.mjs). Dev tooling only; nothing here ships.
//
//   pnpm ui:check                 build both apps, then check
//   pnpm ui:check --no-build      reuse the existing builds
//   pnpm ui:check --only=menu     scenarios whose name contains "menu"
//   pnpm ui:check --app=desktop   one app (extension | desktop)

import { spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { mkdir, readFile, rm, stat } from "node:fs/promises";
import { dirname, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { overflowCheck } from "./check.mjs";
import { extensionScenarios, renderExtension } from "./extension.mjs";
import { desktopScenarios, desktopSetup, scrollTo, tauriStub } from "./desktop.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const OUT = join(root, "ui-check-output");
const EXT_DIST = join(root, "apps/extension/dist/chrome");
const DESK_DIST = join(root, "apps/desktop/dist");

const args = process.argv.slice(2);
const flag = (name) => args.find((a) => a.startsWith(`--${name}=`))?.split("=")[1];
const build = !args.includes("--no-build");
const only = flag("only");
const appFilter = flag("app");

const LOCALES = ["en", "pt-BR"];
const THEMES = ["light", "dark"];
/** tauri.conf.json: the window's minimum size, and its default size. */
const DESKTOP_SIZES = [
  { name: "min", width: 760, height: 500 },
  { name: "default", width: 1040, height: 680 },
];

function run(cmd, cmdArgs) {
  const r = spawnSync(cmd, cmdArgs, { cwd: root, stdio: "inherit" });
  if (r.status !== 0) {
    console.error(`ui-check: ${cmd} ${cmdArgs.join(" ")} failed`);
    process.exit(1);
  }
}

if (build) {
  if (appFilter !== "desktop") run("pnpm", ["--filter", "@havenkeys/extension", "build"]);
  if (appFilter !== "extension") run("pnpm", ["--filter", "@havenkeys/desktop", "exec", "vite", "build", "--logLevel", "warn"]);
}

// ---------------------------------------------------------------- static server
// /ext/* → the Chromium extension build; everything else → the desktop build.

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".woff2": "font/woff2",
  ".woff": "font/woff",
};

const server = createServer(async (req, res) => {
  const url = new URL(req.url ?? "/", "http://localhost");
  let base = DESK_DIST;
  let path = decodeURIComponent(url.pathname);
  if (path.startsWith("/ext/")) {
    base = EXT_DIST;
    path = path.slice(4);
  }
  if (path === "/") path = "/index.html";
  const file = normalize(join(base, path));
  if (!file.startsWith(base)) {
    res.writeHead(403).end();
    return;
  }
  try {
    if (!(await stat(file)).isFile()) throw new Error("not a file");
    res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
    res.end(await readFile(file));
  } catch {
    res.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const baseUrl = `http://127.0.0.1:${server.address().port}`;

// ---------------------------------------------------------------- run

// A full run starts clean; a filtered one only replaces its own screenshots.
if (!only && !appFilter) await rm(OUT, { recursive: true, force: true });
const browser = await chromium.launch();
const failures = [];
let shots = 0;

/** Animations off: screenshots and measurements show the settled layout. */
const STILL = "*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}";

async function checkAndShoot(page, label, file) {
  const found = await page.evaluate(overflowCheck);
  for (const f of found) failures.push({ label, ...f });
  await mkdir(dirname(file), { recursive: true });
  await page.screenshot({ path: file });
  shots++;
}

async function newPage(locale, theme) {
  const context = await browser.newContext({
    locale,
    colorScheme: theme,
    reducedMotion: "reduce",
    deviceScaleFactor: 1,
  });
  // Pin the clock-free bits: navigator.languages follows the context locale.
  const page = await context.newPage();
  page.on("pageerror", (e) => failures.push({ label: page.__label ?? "?", kind: "page-error", selector: "", text: String(e.message).slice(0, 200), detail: "" }));
  return { context, page };
}

if (appFilter !== "desktop") {
  for (const scenario of extensionScenarios) {
    if (only && !scenario.name.includes(only)) continue;
    for (const locale of LOCALES) {
      for (const theme of THEMES) {
        const { context, page } = await newPage(locale, theme);
        const label = `extension/${locale}/${theme}/${scenario.name}`;
        page.__label = label;
        const size = await renderExtension(page, baseUrl, scenario, locale);
        await page.addStyleTag({ content: STILL });
        await checkAndShoot(page, `${label} (${size.width}x${size.height})`, join(OUT, "extension", locale, theme, `${scenario.name}.png`));
        await context.close();
      }
    }
  }
}

if (appFilter !== "extension") {
  for (const scenario of desktopScenarios) {
    if (only && !scenario.name.includes(only)) continue;
    for (const locale of LOCALES) {
      for (const theme of THEMES) {
        for (const size of DESKTOP_SIZES) {
          const { context, page } = await newPage(locale, theme);
          const label = `desktop/${locale}/${theme}/${size.name}/${scenario.name}`;
          page.__label = label;
          await page.setViewportSize({ width: size.width, height: size.height });
          await page.addInitScript(tauriStub, desktopSetup(scenario, locale, theme));
          await page.goto(`${baseUrl}/`);
          await page.evaluate(() => document.fonts.ready);
          await page.addStyleTag({ content: STILL });
          await page.waitForTimeout(250);
          // Before unlocking the app is always dark; after a lock it keeps the
          // vault's theme. Show every screen in both.
          await page.evaluate((t) => (document.documentElement.dataset.theme = t), theme);
          if (scenario.act) {
            try {
              await scenario.act(page);
            } catch (e) {
              failures.push({ label, kind: "script", selector: "", text: String(e.message).split("\n")[0], detail: "" });
            }
            await page.waitForTimeout(300);
          }
          const dir = join(OUT, "desktop", locale, theme, size.name);
          await checkAndShoot(page, label, join(dir, `${scenario.name}.png`));
          for (const [i, target] of (scenario.shots ?? []).entries()) {
            try {
              await scrollTo(page, target);
            } catch (e) {
              failures.push({ label, kind: "script", selector: target, text: String(e.message).split("\n")[0], detail: "" });
              continue;
            }
            await checkAndShoot(page, `${label} @${target}`, join(dir, `${scenario.name}-${i + 2}.png`));
          }
          await context.close();
        }
      }
    }
  }
}

await browser.close();
server.close();

console.log(`ui-check: ${shots} screenshots in ${OUT}`);
if (failures.length > 0) {
  console.error(`ui-check: ${failures.length} problem(s):`);
  for (const f of failures) {
    console.error(`  ${f.label}\n    ${f.kind} ${f.selector} ${f.detail}${f.text ? `\n    “${f.text}”` : ""}`);
  }
  process.exit(1);
}
console.log("ui-check: no clipped text, no horizontal overflow.");
