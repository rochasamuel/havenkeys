// Sites known to support passkeys, from the Passkeys Directory by
// 2factorauth (CC-BY-4.0; see THIRD-PARTY-NOTICES.md). A snapshot committed
// with the extension (scripts/update-passkey-directory.mjs), never fetched.
//
// Only a UI hint: a match shows a help link in the field menu, nothing more,
// so matching is plain host-suffix comparison (no Public Suffix List).
// The data is third-party text: validated here, shown with textContent only.

import raw from "../data/passkey-sites.json";

export interface PasskeySite {
  name: string;
  /** Primary domain first. Lowercase hostnames. */
  domains: string[];
  passwordless: boolean;
  mfa: boolean;
  /** The site's passkey help article, https only. */
  help: string | null;
}

const MAX_NAME = 100;
const MAX_SITES = 10_000;

type Obj = Record<string, unknown>;

function isHostname(v: unknown): v is string {
  if (typeof v !== "string" || v.length > 253 || !v.includes(".") || v.endsWith(".")) return false;
  try {
    return new URL(`https://${v}/`).hostname === v;
  } catch {
    return false;
  }
}

function isHttps(v: unknown): v is string {
  try {
    return typeof v === "string" && new URL(v).protocol === "https:";
  } catch {
    return false;
  }
}

function parseSite(v: unknown): PasskeySite | null {
  if (typeof v !== "object" || v === null || Array.isArray(v)) return null;
  const o = v as Obj;
  const keys = ["name", "domains", "passwordless", "mfa", "help"];
  if (Object.keys(o).length !== keys.length || !keys.every((k) => Object.prototype.hasOwnProperty.call(o, k))) return null;
  const { name, domains, passwordless, mfa, help } = o;
  // eslint-disable-next-line no-control-regex
  if (typeof name !== "string" || name.length === 0 || name.length > MAX_NAME || /[\u0000-\u001f\u007f]/.test(name)) return null;
  if (!Array.isArray(domains) || domains.length === 0 || !domains.every(isHostname)) return null;
  if (typeof passwordless !== "boolean" || typeof mfa !== "boolean") return null;
  if (help !== null && !isHttps(help)) return null;
  return { name, domains: [...domains], passwordless, mfa, help };
}

/** The whole list, or null if any entry is off-shape. */
export function parsePasskeySites(data: unknown): PasskeySite[] | null {
  if (!Array.isArray(data) || data.length > MAX_SITES) return null;
  const out: PasskeySite[] = [];
  for (const v of data) {
    const s = parseSite(v);
    if (!s) return null;
    out.push(s);
  }
  return out;
}

export const PASSKEY_SITES: readonly PasskeySite[] = parsePasskeySites(raw) ?? [];

/** The entry for the page's host: equal to one of its domains or a subdomain of one; the longest domain wins. */
export function findPasskeySite(pageUrl: string, sites: readonly PasskeySite[] = PASSKEY_SITES): PasskeySite | null {
  let host: string;
  try {
    host = new URL(pageUrl).hostname.replace(/\.$/, "");
  } catch {
    return null;
  }
  let best: { site: PasskeySite; len: number } | null = null;
  for (const site of sites) {
    for (const d of site.domains) {
      if ((host === d || host.endsWith(`.${d}`)) && d.length > (best?.len ?? 0)) best = { site, len: d.length };
    }
  }
  return best?.site ?? null;
}
