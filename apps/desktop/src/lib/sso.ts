// "Sign in with" display helpers. The desktop only shows these; which pages
// a login may be used on is decided in Rust.

import type { ItemOverview, SignInWith, SsoProvider } from "./types";

export const PROVIDER_ORDER: SsoProvider[] = ["google", "microsoft", "github", "apple"];
export const PROVIDER_NAMES: Record<SsoProvider, string> = { google: "Google", microsoft: "Microsoft", github: "GitHub", apple: "Apple" };

/** Sites where the provider's own login would be saved. Display only. */
const PROVIDER_DOMAINS: Record<SsoProvider, string[]> = {
  google: ["google.com"],
  microsoft: ["microsoft.com", "microsoftonline.com", "live.com"],
  github: ["github.com"],
  apple: ["apple.com"],
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
