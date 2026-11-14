#!/usr/bin/env node
// Rebuilds crates/havenkeys-core/data/password-rules.json from Apple's
// password-manager-resources (https://github.com/apple/password-manager-resources,
// MIT): quirks/password-rules.json, which maps a domain to the password
// rules its sign-up form does not declare itself. Run by hand on a local
// copy of that file; the output is committed and reviewed like code, and
// nothing fetches it at runtime. The core parses every rule with the
// `password-rules-parser` crate, and its tests fail on one that does not
// parse.
//
// Input:  { "<domain>": { "password-rules": "<rules>" }, ... }
// Output: { "rules": { "<domain>": "<rules>", ... } } (sorted by domain)
//
// Usage: node scripts/update-password-rules.mjs <path/to/password-rules.json>

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const OUT = join(dirname(fileURLToPath(import.meta.url)), "../crates/havenkeys-core/data/password-rules.json");
/** The protocol's bound on a rules string (MAX_PASSWORD_RULES_BYTES). */
const MAX_RULES = 1024;

function hostname(d) {
  if (typeof d !== "string" || d.length === 0 || d.length > 253 || !d.includes(".") || d.endsWith(".")) return null;
  try {
    const h = new URL(`https://${d}/`).hostname;
    return h === d ? h : null;
  } catch {
    return null;
  }
}

const input = process.argv[2];
if (!input) {
  console.error("usage: node scripts/update-password-rules.mjs <password-rules.json>");
  process.exit(2);
}
const data = JSON.parse(readFileSync(input, "utf8"));
const rules = {};
let dropped = 0;
for (const domain of Object.keys(data).sort()) {
  const host = hostname(domain);
  const r = data[domain]?.["password-rules"];
  // eslint-disable-next-line no-control-regex
  if (!host || typeof r !== "string" || r.trim() === "" || Buffer.byteLength(r) > MAX_RULES || /[\u0000-\u001f\u007f]/.test(r)) {
    dropped++;
    continue;
  }
  rules[host] = r.trim();
}
writeFileSync(OUT, `${JSON.stringify({ rules }, null, 2)}\n`);
console.log(`wrote ${Object.keys(rules).length} sites to ${OUT} (${dropped} dropped)`);
