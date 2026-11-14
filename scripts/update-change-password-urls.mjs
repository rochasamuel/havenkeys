#!/usr/bin/env node
// Rebuilds crates/havenkeys-core/data/change-password-urls.json from Apple's
// Password Manager Resources (https://github.com/apple/password-manager-resources,
// quirks/change-password-URLs.json, MIT). Run by hand; the output is
// committed and reviewed like code, and nothing fetches it at runtime.
//
// The file is one object: { "<domain>": "<https change-password page>" }.
// Entries whose key is not a lowercase DNS name, or whose page is not an
// https URL with a host and no credentials, are dropped.
//
// Usage: node scripts/update-change-password-urls.mjs [--from <change-password-URLs.json>]
// Without --from it clones the repository (depth 1) into a temporary folder.

import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "https://github.com/apple/password-manager-resources.git";
const OUT = join(dirname(fileURLToPath(import.meta.url)), "../crates/havenkeys-core/data/change-password-urls.json");
const LABEL = /^(?!-)[a-z0-9-]{1,63}(?<!-)$/;

function domain(d) {
  if (d.length > 253 || !d.includes(".") || !d.split(".").every((l) => LABEL.test(l))) return null;
  try {
    return new URL(`https://${d}/`).hostname === d ? d : null;
  } catch {
    return null;
  }
}

function page(u) {
  if (typeof u !== "string") return null;
  try {
    const p = new URL(u);
    return p.protocol === "https:" && p.hostname && !p.username && !p.password ? p.href : null;
  } catch {
    return null;
  }
}

function convert(raw, source) {
  const out = {};
  let dropped = 0;
  for (const [k, v] of Object.entries(raw).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) {
    const d = domain(k);
    const p = page(v);
    if (!d || !p) {
      dropped += 1;
      console.error(`dropped ${JSON.stringify(k)}`);
      continue;
    }
    out[d] = p;
  }
  writeFileSync(OUT, JSON.stringify(out, null, 2) + "\n");
  console.error(`Wrote ${Object.keys(out).length} sites (${dropped} dropped) from ${source}`);
}

const from = process.argv.indexOf("--from");
if (from !== -1) {
  const file = process.argv[from + 1];
  convert(JSON.parse(readFileSync(file, "utf8")), file);
} else {
  const dir = mkdtempSync(join(tmpdir(), "apple-pmr-"));
  try {
    execFileSync("git", ["clone", "--depth", "1", "--quiet", REPO, dir], { stdio: "inherit" });
    const commit = execFileSync("git", ["-C", dir, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
    const raw = JSON.parse(readFileSync(join(dir, "quirks", "change-password-URLs.json"), "utf8"));
    convert(raw, `apple/password-manager-resources@${commit}`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
