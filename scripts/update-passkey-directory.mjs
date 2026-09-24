#!/usr/bin/env node
// Rebuilds apps/extension/src/data/passkey-sites.json from the Passkeys
// Directory by 2factorauth (https://github.com/2factorauth/passkeys,
// CC-BY-4.0). Run by hand; the output is committed and reviewed like code,
// and the extension never fetches anything at runtime.
//
// The public API (passkeys-api.2fa.directory) carries no site names, so this
// reads the repository's entries/*/*.json: { "<Name>": { "additional-domains"?,
// "passwordless"?, "mfa"?, "documentation"? } }, named after the primary domain.
//
// Usage: node scripts/update-passkey-directory.mjs

import { execFileSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "https://github.com/2factorauth/passkeys.git";
const OUT = join(dirname(fileURLToPath(import.meta.url)), "../apps/extension/src/data/passkey-sites.json");
const MAX_NAME = 100;

function hostname(d) {
  if (typeof d !== "string" || d.length === 0 || d.length > 253 || !d.includes(".") || d.endsWith(".")) return null;
  try {
    const h = new URL(`https://${d}/`).hostname;
    return h === d ? h : null;
  } catch {
    return null;
  }
}

function httpsUrl(u) {
  if (typeof u !== "string") return null;
  try {
    const parsed = new URL(u);
    return parsed.protocol === "https:" ? parsed.href : null;
  } catch {
    return null;
  }
}

function cleanName(n) {
  const t = typeof n === "string" ? n.trim() : "";
  // eslint-disable-next-line no-control-regex
  return t.length > 0 && t.length <= MAX_NAME && !/[\u0000-\u001f\u007f]/.test(t) ? t : null;
}

const dir = mkdtempSync(join(tmpdir(), "passkeys-dir-"));
try {
  execFileSync("git", ["clone", "--depth", "1", "--quiet", REPO, dir], { stdio: "inherit" });
  const commit = execFileSync("git", ["-C", dir, "rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  const sites = [];
  const entries = join(dir, "entries");
  for (const letter of readdirSync(entries)) {
    for (const file of readdirSync(join(entries, letter))) {
      if (!file.endsWith(".json")) continue;
      const primary = hostname(file.slice(0, -".json".length));
      if (!primary) continue;
      const data = JSON.parse(readFileSync(join(entries, letter, file), "utf8"));
      for (const [rawName, e] of Object.entries(data)) {
        const name = cleanName(rawName);
        const passwordless = e?.passwordless === "allowed";
        const mfa = e?.mfa === "allowed";
        if (!name || !(passwordless || mfa)) continue;
        const extra = Array.isArray(e["additional-domains"]) ? e["additional-domains"].map(hostname).filter(Boolean) : [];
        const domains = [...new Set([primary, ...extra])];
        sites.push({ name, domains, passwordless, mfa, help: httpsUrl(e.documentation) });
      }
    }
  }
  sites.sort((a, b) => a.name.localeCompare(b.name, "en") || a.domains[0].localeCompare(b.domains[0]));
  writeFileSync(OUT, JSON.stringify(sites, null, 2) + "\n");
  console.error(`Wrote ${sites.length} sites from 2factorauth/passkeys@${commit}`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
