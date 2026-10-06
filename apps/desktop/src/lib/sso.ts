// "Sign in with" display helpers. The desktop only shows these; which pages
// a login may be used on is decided in Rust.

import type { SecretEdit } from "./secretEdit";
import type { ItemOverview, SignInWith, SsoProvider } from "./types";

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

/** Sites where the provider's own login would be saved. Display only. */
const PROVIDER_DOMAINS: Record<SsoProvider, string[]> = {
  google: ["google.com"],
  microsoft: ["microsoft.com", "microsoftonline.com", "live.com"],
  github: ["github.com"],
  apple: ["apple.com"],
  facebook: ["facebook.com"],
  discord: ["discord.com"],
  x: ["x.com", "twitter.com"],
  linkedin: ["linkedin.com"],
  gitlab: ["gitlab.com"],
};

function hostOf(url: string): string | null {
  try {
    return new URL(url).hostname.toLowerCase();
  } catch {
    return null;
  }
}

/** The vault's login for this item's provider and account, to open from the detail view. */
export function providerLogin(items: ItemOverview[], item: ItemOverview): ItemOverview | null {
  const sso = item.signInWith;
  const account = sso?.account?.trim().toLowerCase();
  if (!sso || !account) return null;
  const domains = PROVIDER_DOMAINS[sso.provider];
  return (
    items.find(
      (o) =>
        o.id !== item.id &&
        o.itemType === "login" &&
        o.username?.trim().toLowerCase() === account &&
        o.urls.some((u) => {
          const h = hostOf(u.url);
          return h !== null && domains.some((d) => h === d || h.endsWith(`.${d}`));
        }),
    ) ?? null
  );
}

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
