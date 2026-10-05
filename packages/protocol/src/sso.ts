// "Sign in with" providers. Mirrors crates/havenkeys-core/src/sso.rs, which
// is the authority (sso-parity.test.ts keeps them equal). The extension uses
// this table only to recognise provider pages while saving; a sign-in run
// continues only into the origins the desktop returns with start_sso.

export type SsoProvider = "google" | "microsoft" | "github" | "apple";

export const SSO_PROVIDERS: Readonly<Record<SsoProvider, { name: string; origins: readonly string[] }>> = {
  google: { name: "Google", origins: ["https://accounts.google.com"] },
  microsoft: { name: "Microsoft", origins: ["https://login.microsoftonline.com", "https://login.live.com"] },
  github: { name: "GitHub", origins: ["https://github.com"] },
  apple: { name: "Apple", origins: ["https://appleid.apple.com"] },
};

export const SSO_PROVIDER_IDS = Object.keys(SSO_PROVIDERS) as readonly SsoProvider[];

export const MAX_ACCOUNT_CHARS = 254;
/** Most vault accounts a check_sso result may offer (Rust: MAX_PROVIDER_ACCOUNTS). */
export const MAX_PROVIDER_ACCOUNTS = 10;

export function isSsoProvider(v: unknown): v is SsoProvider {
  return typeof v === "string" && Object.prototype.hasOwnProperty.call(SSO_PROVIDERS, v);
}

/** Providers whose sign-in pages live on exactly this origin. */
export function providersForOrigin(origin: string): SsoProvider[] {
  return SSO_PROVIDER_IDS.filter((p) => SSO_PROVIDERS[p].origins.includes(origin));
}

/**
 * Provider pages a site embeds as its "Sign in with" button. Google draws
 * the button in an `accounts.google.com/gsi/button` iframe (signed in to
 * Google, it reads "Continue as <name>" with no "Google" in it), so the
 * user's click happens there, not in the site's page. Exact origin + path.
 */
const BUTTON_FRAMES: Readonly<Partial<Record<SsoProvider, readonly string[]>>> = {
  google: ["https://accounts.google.com/gsi/button"],
};

/** The provider whose embedded button frame `url` is, or null. */
export function buttonFrameProvider(url: string): SsoProvider | null {
  let page: string;
  try {
    const u = new URL(url);
    page = u.origin + u.pathname;
  } catch {
    return null;
  }
  for (const p of SSO_PROVIDER_IDS) if (BUTTON_FRAMES[p]?.includes(page)) return p;
  return null;
}
