// Builds dist/chrome and dist/firefox. Output is not minified so the
// shipped code stays readable for review; there are no source maps.

import { build } from "esbuild";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = dirname(fileURLToPath(import.meta.url));
const read = async (p) => JSON.parse(await readFile(join(root, p), "utf8"));

const targets = {
  chrome: { esbuild: "chrome120" },
  firefox: { esbuild: "firefox128" },
};

const base = await read("manifest/base.json");

for (const [browser, t] of Object.entries(targets)) {
  const out = join(root, "dist", browser);
  await rm(out, { recursive: true, force: true });
  await mkdir(out, { recursive: true });

  await build({
    entryPoints: {
      background: join(root, "src/background/index.ts"),
      popup: join(root, "src/popup/popup.ts"),
    },
    outdir: out,
    bundle: true,
    format: "iife",
    target: t.esbuild,
    minify: false,
    sourcemap: false,
    legalComments: "none",
    logLevel: "warning",
  });

  await cp(join(root, "src/popup/popup.html"), join(out, "popup.html"));
  await cp(join(root, "src/popup/popup.css"), join(out, "popup.css"));
  await cp(join(root, "icons"), join(out, "icons"), { recursive: true });

  const manifest = { ...base, ...(await read(`manifest/${browser}.json`)) };
  await writeFile(join(out, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
}
