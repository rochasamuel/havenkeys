// "Sign in with" display helpers. The desktop only shows these; which pages
// a login may be used on is decided in Rust.

import type { SecretEdit } from "./secretEdit";
import type { SignInWith, SsoAccount, SsoProvider } from "./types";

/** Alphabetical by name, as havenkeys-core's `SsoProvider::ALL`. */
export const PROVIDER_ORDER: SsoProvider[] = ["apple", "discord", "facebook", "github", "gitlab", "google", "linkedin", "microsoft", "x"];
export const PROVIDER_NAMES: Record<SsoProvider, string> = {
  apple: "Apple",
  discord: "Discord",
  facebook: "Facebook",
  github: "GitHub",
  gitlab: "GitLab",
  google: "Google",
  linkedin: "LinkedIn",
  microsoft: "Microsoft",
  x: "X",
};

export function ssoSubtitle(s: SignInWith): string {
  return s.account ? `${PROVIDER_NAMES[s.provider]} · ${s.account}` : PROVIDER_NAMES[s.provider];
}

/**
 * Whether picking a provider should tuck the username/password rows away
 * under "Also has a password". Only true for an untouched, empty password
 * on an item that never had one: typed input or an existing password are
 * never collapsed out from under the user.
 */
export function shouldCollapse(username: string, password: SecretEdit, hasExistingPassword: boolean): boolean {
  if (hasExistingPassword) return false;
  if (username.trim() !== "") return false;
  return password.mode === "set" && password.value === "";
}

/** One row of the editor's "Sign in with" picker. */
export type PickerRow =
  | { kind: "provider"; provider: SsoProvider }
  | { kind: "account"; provider: SsoProvider; account: SsoAccount }
  | { kind: "none" };

/**
 * The picker's rows: each provider (in PROVIDER_ORDER) followed by the
 * vault's logins for it, then "None". A query keeps a provider whose name
 * matches (with all its logins) or whose logins match by title or username
 * (with just those); "None" only shows with no query.
 */
export function pickerRows(accounts: Partial<Record<SsoProvider, SsoAccount[]>>, query: string): PickerRow[] {
  const q = query.trim().toLowerCase();
  const rows: PickerRow[] = [];
  for (const provider of PROVIDER_ORDER) {
    const all = accounts[provider] ?? [];
    const nameHit = q === "" || PROVIDER_NAMES[provider].toLowerCase().includes(q);
    const hits = nameHit ? all : all.filter((a) => a.title.toLowerCase().includes(q) || a.username.toLowerCase().includes(q));
    if (!nameHit && hits.length === 0) continue;
    rows.push({ kind: "provider", provider });
    for (const account of hits) rows.push({ kind: "account", provider, account });
  }
  if (q === "") rows.push({ kind: "none" });
  return rows;
}

/** What picking `row` sets "Sign in with" to. */
export function applyRow(row: PickerRow, current: SignInWith | null): SignInWith | null {
  switch (row.kind) {
    case "none":
      return null;
    case "account":
      return { provider: row.provider, account: row.account.username };
    case "provider":
      return { provider: row.provider, account: current?.provider === row.provider ? current.account : null };
  }
}
