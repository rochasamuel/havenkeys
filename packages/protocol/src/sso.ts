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

export function isSsoProvider(v: unknown): v is SsoProvider {
  return typeof v === "string" && Object.prototype.hasOwnProperty.call(SSO_PROVIDERS, v);
}

/** Providers whose sign-in pages live on exactly this origin. */
export function providersForOrigin(origin: string): SsoProvider[] {
  return SSO_PROVIDER_IDS.filter((p) => SSO_PROVIDERS[p].origins.includes(origin));
}
